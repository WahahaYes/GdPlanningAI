//! Plan scheduler exposed to GDScript.
//!
//! [`GdPAIPlanScheduler`] owns a Rayon thread pool and a callable registry.
//! GDScript agents submit planning jobs via [`submit_plan`]; each frame the
//! autoload calls [`process_callbacks`] to drain pending callback requests
//! from planner threads and deliver completed results.

use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::planner::{
    DijkstraHeuristic, PlannerEngine, ProvisionKind, SearchContext, TerminationStrategy,
};
use crate::precondition::{PreconditionHandler, PreconditionOp};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use godot::prelude::*;
use std::collections::{HashMap, HashSet};
use std::sync::mpsc::{Receiver, Sender};
use std::sync::{Arc, atomic::AtomicBool};

struct ActiveJobHandle {
    agent: Gd<Object>,
    agent_instance_id: i64,
    callable_registry: Vec<Callable>,
    request_rx: Receiver<CallbackRequest>,
    result_rx: Receiver<(PlannerRunResult, PlannerEngine)>,
    result_tx: Sender<(PlannerRunResult, PlannerEngine)>,
    engine: Option<PlannerEngine>,
    goals: Vec<GoalSpec>,
    /// If all goals were already satisfied by the initial state, this is the
    /// original index of the highest-reward satisfied goal; otherwise -1.
    satisfied_goal_index: i64,
    cancel_flag: Arc<AtomicBool>,
    pending_request_id: usize,
    done: bool,
    pending_reap: bool, // New flag
    completed_request_ids: HashSet<usize>,
    stall_notice: StallNoticeThrottle,
}

/// Rate limiter for the "NOT resuming" stall notice in [`process_callbacks`].
///
/// A stalled job keeps the same `pending_request_id` every frame, so logging
/// unconditionally would spam once per frame for the whole stall. The throttle
/// emits one line per stall episode (per distinct pending id) so stalls stay
/// visible without hiding behind — or drowning in — repetition.
#[derive(Default)]
pub struct StallNoticeThrottle {
    last_logged_id: Option<usize>,
}

impl StallNoticeThrottle {
    /// Returns true the first time each distinct `pending_id` is seen.
    pub fn should_log(&mut self, pending_id: usize) -> bool {
        if self.last_logged_id == Some(pending_id) {
            return false;
        }
        self.last_logged_id = Some(pending_id);
        true
    }

    /// Clears the remembered id once the job resumes or finishes waiting.
    pub fn reset(&mut self) {
        self.last_logged_id = None;
    }
}

/// Planning scheduler.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct GdPAIPlanScheduler {
    /// Maximum Rayon worker threads. 0 = number of logical CPUs.
    #[export]
    max_threads: i64,
    active_jobs: Vec<ActiveJobHandle>,
    thread_pool: Option<rayon::ThreadPool>,
    base: Base<Node>,
}

#[godot_api]
impl INode for GdPAIPlanScheduler {
    fn init(base: Base<Node>) -> Self {
        Self {
            max_threads: 0,
            active_jobs: Vec::new(),
            thread_pool: None,
            base,
        }
    }

    fn ready(&mut self) {
        let mut builder = rayon::ThreadPoolBuilder::new();
        if self.max_threads > 0 {
            builder = builder.num_threads(self.max_threads as usize);
        }
        self.thread_pool = Some(
            builder
                .build()
                .expect("GdPAIPlanScheduler: failed to build Rayon thread pool"),
        );
        log_info!("GdPAIPlanScheduler ready");
    }
}

#[godot_api]
impl GdPAIPlanScheduler {
    /// Drain pending callback requests and deliver completed results.
    /// Call this once per frame from GDScript `_process`.
    #[func]
    fn process_callbacks(&mut self) {
        crate::logger::process_logs();

        // 1. Recover engines from worker threads first.
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            while let Ok((run_result, engine)) = job.result_rx.try_recv() {
                // Always put the engine back into the handle so we can extract its debug tree
                job.engine = Some(engine);

                match run_result {
                    PlannerRunResult::Complete(result) => {
                        job.done = true;
                        if job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            log_debug!(
                                "Job for agent instance {} cancelled successfully",
                                job.agent_instance_id
                            );
                            continue;
                        }

                        let Some(result) = result else {
                            // If all goals were already satisfied by the initial state,
                            // return an empty, successful plan pointing at the highest-reward
                            // satisfied goal. Otherwise report a failed search.
                            if job.agent.is_instance_valid() {
                                let res = if job.satisfied_goal_index >= 0 {
                                    PlanResult {
                                        success: true,
                                        action_chain: vec![],
                                        total_cost: 0.0,
                                        goal_index: job.satisfied_goal_index,
                                        deferred_action_indices: vec![],
                                        action_bindings: vec![],
                                    }
                                } else {
                                    PlanResult {
                                        success: false,
                                        action_chain: vec![],
                                        total_cost: 0.0,
                                        goal_index: -1,
                                        deferred_action_indices: vec![],
                                        action_bindings: vec![],
                                    }
                                };
                                log_info!(
                                    "Plan complete: success={}, actions=0, cost=0.0 (no search ran)",
                                    res.success
                                );
                                let dict = result_to_dict(&res);
                                job.agent.call("_on_plan_ready", &[dict.to_variant()]);
                            }
                            continue;
                        };

                        // If every goal was already satisfied, the engine returned an empty
                        // failed plan because it had no goals to search. Report success.
                        let mut result = result;
                        if !result.success && job.satisfied_goal_index >= 0 {
                            result.success = true;
                            result.goal_index = job.satisfied_goal_index;
                            result.total_cost = 0.0;
                        }

                        if job.agent.is_instance_valid() {
                            log_info!(
                                "Plan complete: success={}, actions={}, cost={:.1}",
                                result.success,
                                result.action_chain.len(),
                                result.total_cost
                            );
                            let dict = result_to_dict(&result);
                            job.agent.call("_on_plan_ready", &[dict.to_variant()]);
                        }
                    }
                    PlannerRunResult::Pending(id) => {
                        log_debug!(
                            "Engine yielded Pending({}) for agent instance {}",
                            id,
                            job.agent_instance_id
                        );
                        job.pending_request_id = id;
                    }
                }
            }
        }

        // 2. Process pending requests from Godot
        let mut jobs_with_responses = HashSet::new();
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            while let Ok(req) = job.request_rx.try_recv() {
                let callable = &job.callable_registry[req.callable_id];
                let response = dispatch_callback(callable, req.kind, &req.bindings);

                let _ = req.response_tx.send(PlannerCallback {
                    request_id: req.request_id,
                    response,
                });
                job.completed_request_ids.insert(req.request_id);
                jobs_with_responses.insert(job.agent_instance_id);
            }
        }

        // 3. Resume engines that are ready
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            // If job was cancelled while yielded, mark as done and don't resume
            if job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                job.done = true;
                continue;
            }

            let ready_to_resume = if job.pending_request_id == 0 {
                // Yielded for budget, always ready
                true
            } else {
                // Yielded for callback, ready if we got a response this frame
                // OR if we already sent a response for this specific request ID
                // in a previous frame (the response may have arrived after the
                // engine passed its response_rx drain).
                let has_this_frame = jobs_with_responses.contains(&job.agent_instance_id);
                let has_previous = job.completed_request_ids.contains(&job.pending_request_id);
                if !has_this_frame
                    && !has_previous
                    && job.stall_notice.should_log(job.pending_request_id)
                {
                    log_debug!(
                        "NOT resuming agent instance {}: pending_request_id={} but no response received",
                        job.agent_instance_id,
                        job.pending_request_id
                    );
                }
                has_this_frame || has_previous
            };

            if ready_to_resume && let Some(engine) = job.engine.take() {
                log_debug!(
                    "Resuming search for agent instance {}",
                    job.agent_instance_id
                );
                job.stall_notice.reset();
                job.completed_request_ids.clear();
                let goals = job.goals.clone();
                let res_tx = job.result_tx.clone();
                run_job_step(self.thread_pool.as_ref(), goals, res_tx, engine);
            }
        }

        // Clean up finished jobs that have had one frame to be inspected
        self.active_jobs.retain(|job| !job.pending_reap);
        for job in self.active_jobs.iter_mut().filter(|j| j.done) {
            job.pending_reap = true;
        }

        crate::logger::process_logs();
    }

    /// Submit a planning job for `agent`.
    ///
    /// `time_slice_ms` is the wall-clock budget (ms) per search step;
    /// clamped to >= 0 where 0 disables time-slicing.
    #[func]
    #[allow(clippy::too_many_arguments)]
    fn submit_plan(
        &mut self,
        agent: Gd<Object>,
        agent_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        world_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        actions: Array<VarDictionary>,
        goals: Array<VarDictionary>,
        max_recursion: i64,
        iteration_budget: i64,
        time_slice_ms: i64,
    ) {
        let agent_instance_id = agent.instance_id().to_i64();

        for job in self
            .active_jobs
            .iter_mut()
            .filter(|j| !j.done && j.agent_instance_id == agent_instance_id)
        {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());
        let mut initial_provisions = crate::requirement::extract_initial_provisions(&snap_agent);
        initial_provisions.extend(extract_provisions_from_snapshot(&snap_world));

        let mut job_registry = Vec::new();
        let action_specs = build_action_specs(&actions, &mut job_registry);
        let mut goal_specs = build_goal_specs(&goals, &mut job_registry);

        // Detect the highest-reward goal already satisfied by the initial state.
        // If all goals are satisfied, the scheduler can return an empty success
        // plan instead of asking the planner to search.
        let mut satisfied_goal_index = -1i64;
        let mut best_satisfied_reward = f64::NEG_INFINITY;
        let goal_satisfied = |goal: &GoalSpec| -> bool {
            goal.desired_state.iter().all(|pre| {
                PreconditionHandler::from_spec(pre, &job_registry)
                    .is_some_and(|h| h.evaluate(&agent_bb, &world_bb))
            })
        };
        for goal in &goal_specs {
            if goal_satisfied(goal) && goal.reward > best_satisfied_reward {
                best_satisfied_reward = goal.reward;
                satisfied_goal_index = goal.original_index as i64;
            }
        }

        // Drop goals already satisfied by the initial state so the planner never
        // searches for a goal that is already achieved.
        goal_specs.retain(|goal| !goal_satisfied(goal));

        // If every goal was already satisfied, the index is meaningful; otherwise
        // the planner will search the remaining unsatisfied goals.
        if !goal_specs.is_empty() {
            satisfied_goal_index = -1;
        }

        // Sort goals by reward descending
        goal_specs.sort_by(|a, b| {
            b.reward
                .partial_cmp(&a.reward)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let goal_names: Vec<&str> = goal_specs.iter().map(|g| g.name.as_str()).collect();
        log_info!(
            "submit_plan: agent={} goals=[{}] actions={}",
            agent_instance_id,
            goal_names.join(", "),
            actions.len()
        );

        let (req_tx, req_rx) = std::sync::mpsc::channel::<CallbackRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<(PlannerRunResult, PlannerEngine)>();
        let (engine_tx, engine_rx) = std::sync::mpsc::channel::<PlannerCallback>();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let max_rec = max_recursion.max(1) as usize;
        let iter_budget = iteration_budget.max(100) as usize;
        let slice_ms = time_slice_ms.max(0) as u64;
        if max_recursion < 1 {
            log_warn!("max_recursion was clamped from {} to 1", max_recursion);
        }
        if iteration_budget < 100 {
            log_warn!(
                "iteration_budget was clamped from {} to 100",
                iteration_budget
            );
        }
        if time_slice_ms < 0 {
            log_warn!(
                "time_slice_ms was clamped from {} to 0 (disabled)",
                time_slice_ms
            );
        }

        // Build provision index for fast candidate discovery.
        let mut provision_index: HashMap<(ProvisionKind, String), Vec<usize>> = HashMap::new();
        let mut non_wildcard_actions = Vec::new();
        for (idx, action) in action_specs.iter().enumerate() {
            let has_wildcard = action
                .provisions
                .iter()
                .any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }));
            if !has_wildcard {
                non_wildcard_actions.push(idx);
            }
            for prov in &action.provisions {
                let (kind, name) = match prov {
                    ProvisionSpec::Binding { binding_name, .. } => {
                        (ProvisionKind::Binding, binding_name.clone())
                    }
                    ProvisionSpec::Fact { fact_name, .. } => {
                        (ProvisionKind::Fact, fact_name.clone())
                    }
                    ProvisionSpec::FactWildcard { fact_name } => {
                        (ProvisionKind::FactWildcard, fact_name.clone())
                    }
                };
                provision_index.entry((kind, name)).or_default().push(idx);
            }
        }

        let ctx = Arc::new(SearchContext {
            actions: action_specs,
            initial_agent: snap_agent,
            initial_world: snap_world,
            initial_provisions,
            request_tx: req_tx,
            engine_response_tx: engine_tx.clone(),
            discovery_results: std::sync::Mutex::new(HashMap::new()),
            discovery_costs: std::sync::Mutex::new(HashMap::new()),
            discovery_pending: std::sync::Mutex::new(HashMap::new()),
            discovery_request_map: std::sync::Mutex::new(HashMap::new()),
            discovery_precond_results: std::sync::Mutex::new(HashMap::new()),
            discovery_precond_pending: std::sync::Mutex::new(HashMap::new()),
            provision_index,
            non_wildcard_actions,
        });

        let mut engine = PlannerEngine::new(ctx, max_rec, cancel_flag.clone())
            .with_heuristic(Box::new(DijkstraHeuristic))
            .with_termination_strategy(TerminationStrategy::BestCost)
            .with_iteration_budget(iter_budget)
            .with_time_slice_ms(slice_ms);

        // Use the channel we created
        engine.response_rx = engine_rx;
        engine.response_tx = engine_tx.clone();

        let job = ActiveJobHandle {
            agent,
            agent_instance_id,
            callable_registry: job_registry,
            request_rx: req_rx,
            result_rx: res_rx,
            result_tx: res_tx.clone(),
            engine: None,
            goals: goal_specs,
            satisfied_goal_index,
            cancel_flag,
            pending_request_id: 0,
            done: false,
            pending_reap: false,
            completed_request_ids: HashSet::new(),
            stall_notice: StallNoticeThrottle::default(),
        };

        run_job_step(self.thread_pool.as_ref(), job.goals.clone(), res_tx, engine);
        self.active_jobs.push(job);
    }

    /// Number of jobs currently in flight.
    #[func]
    fn active_job_count(&self) -> i64 {
        self.active_jobs.len() as i64
    }

    /// Cancel all in-flight planning jobs for a specific agent.
    #[func]
    fn cancel_agent_jobs(&mut self, agent: Gd<Object>) {
        let agent_instance_id = agent.instance_id().to_i64();
        for job in &mut self.active_jobs {
            if job.agent_instance_id == agent_instance_id {
                job.cancel_flag
                    .store(true, std::sync::atomic::Ordering::Relaxed);
                log_debug!(
                    "Cancelled planning job for agent instance {}",
                    job.agent_instance_id
                );
            }
        }
    }

    /// Clear all active jobs from the scheduler.
    #[func]
    fn clear_active_jobs(&mut self) {
        for job in &mut self.active_jobs {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.active_jobs.clear();
        log_debug!("Cleared all active jobs from scheduler");
    }

    /// Signal all active jobs to cancel but do not remove them from the registry.
    /// This allows process_callbacks to still recover the engines for debug tree inspection.
    #[func]
    fn cancel_all_jobs(&mut self) {
        for job in &mut self.active_jobs {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        log_debug!("Signalled all active jobs to cancel");
    }

    /// Returns a string describing the current state of the thread pool.
    #[func]
    fn get_pool_status(&self) -> String {
        if let Some(pool) = &self.thread_pool {
            format!(
                "Threads: {} | Active Jobs: {}",
                pool.current_num_threads(),
                self.active_jobs.len(),
            )
        } else {
            "Pool not initialized".to_string()
        }
    }

    /// Sets the process-wide log verbosity.
    #[func]
    fn set_log_level(&self, level: i64) {
        let log_level = crate::logger::LogLevel::from_u8(level.clamp(0, 3) as u8);
        crate::logger::set_log_level(log_level);
    }

    /// Returns a human-readable search tree for the given agent's current planning job.
    /// Returns an empty string if no active job is found or if the job is currently running in a thread.
    #[func]
    fn get_debug_tree(&self, agent: Gd<Object>) -> String {
        let agent_instance_id = agent.instance_id().to_i64();
        // First, prefer a non-cancelled job
        for job in &self.active_jobs {
            if job.agent_instance_id == agent_instance_id
                && !job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed)
            {
                if let Some(engine) = &job.engine {
                    return engine.tree.format();
                } else {
                    return "Engine is currently running in a background thread.".to_string();
                }
            }
        }
        // Fall back to any matching job (including cancelled)
        for job in &self.active_jobs {
            if job.agent_instance_id == agent_instance_id {
                if let Some(engine) = &job.engine {
                    return engine.tree.format();
                } else {
                    return "Engine is currently running in a background thread.".to_string();
                }
            }
        }
        String::new()
    }
}

fn run_job_step(
    thread_pool: Option<&rayon::ThreadPool>,
    goals: Vec<GoalSpec>,
    res_tx: Sender<(PlannerRunResult, PlannerEngine)>,
    engine: PlannerEngine,
) {
    if let Some(tp) = thread_pool {
        let mut engine_mut = engine;
        tp.spawn(move || {
            let result = engine_mut.plan(&goals);
            let _ = res_tx.send((result, engine_mut));
        });
    }
}

fn register_callable(registry: &mut Vec<Callable>, callable: Callable) -> usize {
    let id = registry.len();
    registry.push(callable);
    id
}

fn build_action_specs(
    actions: &Array<VarDictionary>,
    registry: &mut Vec<Callable>,
) -> Vec<ActionSpec> {
    actions
        .iter_shared()
        .filter_map(|dict| {
            let name = match dict.get("name").and_then(|v| v.try_to::<String>().ok()) {
                Some(n) => n,
                None => {
                    log_warn!("Skipping action dictionary: missing or invalid 'name' field");
                    return None;
                }
            };

            let cost_val = dict.get("cost_callable");
            let cost_id = cost_val.as_ref().and_then(|v| {
                if let Ok(c) = v.try_to::<Callable>() {
                    if c.is_valid() {
                        Some(register_callable(registry, c))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            let effect_val = dict.get("effect_callable");
            let effect_id = effect_val.as_ref().and_then(|v| {
                if let Ok(c) = v.try_to::<Callable>() {
                    if c.is_valid() {
                        Some(register_callable(registry, c))
                    } else {
                        None
                    }
                } else {
                    None
                }
            });

            let preconditions = extract_precond_specs(&dict, "preconditions", registry);
            let validity_checks = extract_precond_specs(&dict, "validity_checks", registry);
            let requirements = extract_requirement_specs(&dict, "requirements");
            let provisions = extract_provision_specs(&dict, "provisions");

            let mut dependent_object_ids: Vec<i64> = Vec::new();
            for precond in &preconditions {
                dependent_object_ids.extend_from_slice(precond.dependent_object_ids());
            }
            for check in &validity_checks {
                dependent_object_ids.extend_from_slice(check.dependent_object_ids());
            }

            if let Some(action_deps) = dict
                .get("dependent_object_ids")
                .and_then(|v| v.try_to::<Array<Variant>>().ok())
            {
                dependent_object_ids.extend(
                    action_deps
                        .iter_shared()
                        .filter_map(|v| v.try_to::<i64>().ok()),
                );
            }

            Some(ActionSpec {
                name,
                cost_callable_id: cost_id,
                effect_callable_id: effect_id,
                preconditions,
                validity_checks,
                requirements,
                provisions,
                dependent_object_ids,
            })
        })
        .collect()
}

fn build_goal_specs(goals: &Array<VarDictionary>, registry: &mut Vec<Callable>) -> Vec<GoalSpec> {
    goals
        .iter_shared()
        .enumerate()
        .filter_map(|(idx, dict)| {
            let name = match dict.get("name").and_then(|v| v.try_to::<String>().ok()) {
                Some(n) => n,
                None => {
                    log_warn!(
                        "Skipping goal dictionary at index {}: missing or invalid 'name' field",
                        idx
                    );
                    return None;
                }
            };
            let reward = match dict.get("reward").and_then(|v| v.try_to::<f64>().ok()) {
                Some(r) => r,
                None => {
                    log_warn!(
                        "Skipping goal '{}': missing or invalid 'reward' field",
                        name
                    );
                    return None;
                }
            };
            let desired_state = extract_precond_specs(&dict, "desired_state", registry);
            Some(GoalSpec {
                name,
                reward,
                desired_state,
                original_index: idx,
            })
        })
        .collect()
}

fn extract_typed_specs<T, F>(dict: &VarDictionary, key: &str, mut parse_fn: F) -> Vec<T>
where
    F: FnMut(&VarDictionary) -> Option<T>,
{
    dict.get(key)
        .and_then(|v| {
            v.try_to::<Array<VarDictionary>>().ok().or_else(|| {
                v.try_to::<VarArray>().ok().map(|arr| {
                    let mut typed = Array::<VarDictionary>::new();
                    for item in arr.iter_shared() {
                        if let Ok(dict) = item.try_to::<VarDictionary>() {
                            typed.push(&dict);
                        }
                    }
                    typed
                })
            })
        })
        .map(|arr| arr.iter_shared().filter_map(|d| parse_fn(&d)).collect())
        .unwrap_or_default()
}

fn extract_precond_specs(
    dict: &VarDictionary,
    key: &str,
    registry: &mut Vec<Callable>,
) -> Vec<PreconditionSpec> {
    extract_typed_specs(dict, key, |d| precond_spec_from_dict(d, registry))
}

fn extract_requirement_specs(dict: &VarDictionary, key: &str) -> Vec<RequirementSpec> {
    extract_typed_specs(dict, key, RequirementSpec::from_dict)
}

fn extract_provision_specs(dict: &VarDictionary, key: &str) -> Vec<ProvisionSpec> {
    extract_typed_specs(dict, key, ProvisionSpec::from_dict)
}

fn precond_spec_from_dict(
    dict: &VarDictionary,
    registry: &mut Vec<Callable>,
) -> Option<PreconditionSpec> {
    let handler = PreconditionHandler::from_dict(dict)?;

    if handler.operation == PreconditionOp::CustomCallback {
        let callable = handler.eval_callable?;
        let id = register_callable(registry, callable);

        let dependent_object_ids = dict
            .get("dependent_object_ids")
            .and_then(|v| v.try_to::<Array<Variant>>().ok())
            .map(|arr| {
                arr.iter_shared()
                    .filter_map(|v| v.try_to::<i64>().ok())
                    .collect()
            })
            .unwrap_or_default();

        return Some(PreconditionSpec::Custom {
            callable_id: id,
            dependent_object_ids,
        });
    }

    Some(PreconditionSpec::Builtin {
        target: handler.target,
        operation: handler.operation,
        property_name: handler.property_name,
        value: handler.value.map(|v| VariantSnapshot::from_variant(&v)),
    })
}

fn dispatch_callback(
    callable: &Callable,
    kind: CallbackKind,
    bindings: &[(String, Vec<VariantSnapshot>)],
) -> CallbackResponse {
    match kind {
        CallbackKind::GetCost { agent, world } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();

            inject_bindings_into_agent(&mut bb_agent, bindings);

            let args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            let result = callable.call(&args);
            let cost = if let Ok(f) = result.try_to::<f64>() {
                f
            } else if let Ok(i) = result.try_to::<i64>() {
                i as f64
            } else {
                log_warn!(
                    "Cost callable {} returned unexpected type {:?} (value: {:?}); treating as infinite cost",
                    callable.to_string(),
                    result.get_type(),
                    result
                );
                f64::INFINITY
            };
            CallbackResponse::Float(cost)
        }
        CallbackKind::ApplyEffect { agent, world } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();

            inject_bindings_into_agent(&mut bb_agent, bindings);

            let args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            callable.call(&args);

            let new_agent = BlackboardSnapshot::from_blackboard(&bb_agent.bind());
            let new_world = BlackboardSnapshot::from_blackboard(&bb_world.bind());
            CallbackResponse::UpdatedSnapshots(new_agent, new_world)
        }
        CallbackKind::EvalCustomPrecond { agent, world } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();

            inject_bindings_into_agent(&mut bb_agent, bindings);

            let args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            let result = callable.call(&args);
            let bool_result = match result.try_to::<bool>() {
                Ok(b) => b,
                Err(_) => {
                    log_warn!(
                        "Custom precondition callable {} returned non-bool type {:?} (value: {:?}); treating as false",
                        callable.to_string(),
                        result.get_type(),
                        result
                    );
                    false
                }
            };
            CallbackResponse::Bool(bool_result)
        }
    }
}

/// Injects bindings into the agent blackboard only. World-state bindings are not currently supported.
fn inject_bindings_into_agent(
    agent_bb: &mut Gd<GdPAIBlackboard>,
    bindings: &[(String, Vec<VariantSnapshot>)],
) {
    let mut bind = agent_bb.bind_mut();
    for (name, values) in bindings {
        if !values.is_empty() {
            bind.properties.insert(name.clone(), values[0].to_variant());
        }
    }
}

fn extract_provisions_from_snapshot(snap: &BlackboardSnapshot) -> Vec<ProvisionSpec> {
    let mut provisions = Vec::new();
    for (uid, obj) in &snap.objects {
        for (prop_name, val) in &obj.properties {
            if prop_name == "provides"
                && let VariantSnapshot::Str(fact_name) = val
            {
                let mut args = Vec::new();
                if let Ok(id) = uid.parse::<i64>() {
                    args.push(VariantSnapshot::Int(id));
                }
                provisions.push(ProvisionSpec::Fact {
                    fact_name: fact_name.clone(),
                    args,
                });
            }
        }
    }
    provisions
}

fn result_to_dict(result: &PlanResult) -> VarDictionary {
    let mut dict = VarDictionary::new();
    dict.set("success", result.success);
    dict.set("total_cost", result.total_cost);
    dict.set("goal_index", result.goal_index);

    let mut action_chain = Array::<Variant>::new();
    for action_index in &result.action_chain {
        action_chain.push(&action_index.to_variant());
    }
    dict.set("action_chain", action_chain.to_variant());

    // Add action-specific bindings
    let mut action_bindings = Array::<Variant>::new();
    for (chain_position, fact_name, values) in &result.action_bindings {
        let mut binding_tuple = Array::<Variant>::new();
        binding_tuple.push(&chain_position.to_variant());
        binding_tuple.push(&fact_name.to_variant());
        let mut vals_array = Array::<Variant>::new();
        for val in values {
            vals_array.push(&val.to_variant());
        }
        binding_tuple.push(&vals_array.to_variant());
        action_bindings.push(&binding_tuple.to_variant());
    }
    dict.set("action_bindings", action_bindings.to_variant());

    dict
}

#[cfg(test)]
mod tests {
    use super::StallNoticeThrottle;

    #[test]
    fn first_sight_of_pending_id_logs() {
        let mut throttle = StallNoticeThrottle::default();
        assert!(throttle.should_log(7));
    }

    #[test]
    fn repeated_pending_id_is_suppressed() {
        let mut throttle = StallNoticeThrottle::default();
        assert!(throttle.should_log(7));
        assert!(!throttle.should_log(7));
        assert!(!throttle.should_log(7));
    }

    #[test]
    fn new_pending_id_logs_again() {
        let mut throttle = StallNoticeThrottle::default();
        assert!(throttle.should_log(7));
        assert!(!throttle.should_log(7));
        assert!(throttle.should_log(9));
        assert!(!throttle.should_log(9));
    }

    #[test]
    fn reset_reenables_logging() {
        let mut throttle = StallNoticeThrottle::default();
        assert!(throttle.should_log(7));
        throttle.reset();
        assert!(throttle.should_log(7));
    }
}
