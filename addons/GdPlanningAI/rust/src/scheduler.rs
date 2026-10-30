//! Plan scheduler exposed to GDScript: owns a Rayon pool and one [`PlannerEngine`] per job.
//!
//! `submit_plan` (main thread) converts GDScript data into Send-safe specs. The engine is moved
//! into a worker for each step and sent back afterwards, so it is never shared: `engine` is `None`
//! while a step runs. Each frame `process_callbacks` collects finished steps, runs requested
//! GDScript callables, resumes engines that can progress, and reaps finished jobs (dropping
//! engines off the main thread). Cancellation sets a flag the engine checks between iterations.

use crate::effect_ir::EffectOp;
use crate::gdpai_blackboard::GdPAIBlackboard;
use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::planner::{EngineConfig, EngineInputs, PlannerEngine};
use crate::precondition::{PreconditionHandler, PreconditionOp};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use godot::prelude::*;
use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender};

/// Main-thread bookkeeping for one planning job. Lives in
/// [`GdPAIPlanScheduler::active_jobs`] from `submit_plan` until the job is reaped.
struct ActiveJobHandle {
    /// The agent object that receives `_on_plan_ready`. Held as a `Gd` and checked with
    /// `is_instance_valid` before use, since the agent may be freed mid-plan.
    agent: Gd<Object>,
    /// Instance id of `agent`, used to find and cancel an agent's earlier jobs.
    agent_instance_id: i64,
    /// Every `Callable` of this job; the planner refers to them by index. Only touched on the
    /// main thread because `Callable` is not `Send`.
    callable_registry: Vec<Callable>,
    /// Requests from the planner thread for GDScript callables to be run.
    request_rx: Receiver<CallbackRequest>,
    /// Engines coming back from worker threads, with the result of the step they just ran.
    result_rx: Receiver<(PlannerRunResult, PlannerEngine)>,
    /// Cloned into each worker step so it can hand the engine back.
    result_tx: Sender<(PlannerRunResult, PlannerEngine)>,
    /// The engine while it is parked between steps; `None` while a worker owns it.
    engine: Option<PlannerEngine>,
    /// Goals still to be searched (satisfied ones removed, sorted by reward, highest first).
    goals: Vec<GoalSpec>,
    /// If all goals were already satisfied by the initial state, this is the
    /// original index of the highest-reward satisfied goal; otherwise -1.
    satisfied_goal_index: i64,
    /// Shared with the engine; set to abort the job.
    cancel_flag: Arc<AtomicBool>,
    /// What the last finished step is waiting for: a request id, or 0 when it only yielded its
    /// budget and can be resumed immediately.
    pending_request_id: usize,
    /// The job will not run again (result delivered, failed, or cancelled).
    done: bool,
    /// The job was already `done` a frame ago and may now be dropped. The extra frame gives
    /// GDScript a chance to read the engine's debug tree right after the result arrives.
    pending_reap: bool,
    /// Requests answered since the last resume, so an answer arriving after the engine drained
    /// its channel still triggers a resume.
    completed_request_ids: HashSet<usize>,
    /// Limits the "not resuming" debug notice to once per stalled request.
    stall_notice: StallNoticeThrottle,
}

/// Logs the "NOT resuming" stall notice once per pending id instead of every frame.
#[derive(Default)]
pub(crate) struct StallNoticeThrottle {
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

/// Runs planning jobs on background threads and services their GDScript callbacks. Meant as an
/// autoload: call `submit_plan` to start a job and `process_callbacks` every frame, or no job
/// progresses. The thread pool is created in `_ready`.
#[derive(GodotClass)]
#[class(base=Node)]
pub struct GdPAIPlanScheduler {
    /// Maximum Rayon worker threads. 0 = number of logical CPUs. Read once when the node
    /// becomes ready; later changes have no effect.
    #[export]
    max_threads: i64,
    /// Jobs in flight or waiting to be reaped.
    active_jobs: Vec<ActiveJobHandle>,
    /// Worker pool; `None` until `ready`.
    thread_pool: Option<rayon::ThreadPool>,
    /// Native effect applications across all jobs; incremented on planner threads.
    effect_ir_hits: Arc<AtomicU64>,
    /// Effect callbacks that fell back to GDScript; incremented on the main thread.
    effect_callback_fallbacks: u64,
    /// Jobs that tripped the fallback budget (once per job, with the warn). Shared with
    /// engines so the dispatch stats expose budget pressure.
    effect_fallback_trips: Arc<AtomicU64>,
    /// Last totals covered by the debug summary, so it only logs on change.
    last_dispatch_log: (u64, u64, u64),
    base: Base<Node>,
}

#[godot_api]
impl INode for GdPAIPlanScheduler {
    fn init(base: Base<Node>) -> Self {
        Self {
            max_threads: 0,
            active_jobs: Vec::new(),
            thread_pool: None,
            effect_ir_hits: Arc::new(AtomicU64::new(0)),
            effect_callback_fallbacks: 0,
            effect_fallback_trips: Arc::new(AtomicU64::new(0)),
            last_dispatch_log: (0, 0, 0),
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
    /// Call once per frame: prints queued logs, runs requested callables, resumes engines, and
    /// delivers results to the agent's `_on_plan_ready` as a dictionary (`success`, `total_cost`,
    /// `goal_index`, `action_chain`, `action_bindings`). Cancelled jobs deliver nothing.
    #[func]
    fn process_callbacks(&mut self) {
        crate::logger::process_logs();

        // 1. Recover engines from worker threads first. The engine is only ever readable here
        // because the worker moves it back through the channel when its step ends.
        // (`done` jobs are skipped: an engine that is still on a worker, or arrives later, is
        // dropped together with the job's channel when the job is reaped.)
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

                        // `None` only comes from cancellation, handled above.
                        let Some(mut result) = result else {
                            continue;
                        };

                        // If every goal was already satisfied, the engine returned an empty
                        // failed plan because it had no goals to search. Report success.
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

        // 2. Run requested GDScript callables. This is the only place callables execute, which
        // is what keeps them on the main thread. The answer goes directly to the engine's own
        // receiver; the engine reads it on its next step.
        let mut jobs_with_responses = HashSet::new();
        let mut effect_fallbacks_this_frame = 0u64;
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            while let Ok(req) = job.request_rx.try_recv() {
                if matches!(req.kind, CallbackKind::ApplyEffect { .. }) {
                    effect_fallbacks_this_frame += 1;
                }
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
        self.effect_callback_fallbacks += effect_fallbacks_this_frame;
        let ir_hits = self.effect_ir_hits.load(Ordering::Relaxed);
        let trips = self.effect_fallback_trips.load(Ordering::Relaxed);
        if (ir_hits, self.effect_callback_fallbacks, trips) != self.last_dispatch_log {
            self.last_dispatch_log = (ir_hits, self.effect_callback_fallbacks, trips);
            log_debug!(
                "Effect dispatch totals: native_ir={} callback_fallbacks={} fallback_trips={}",
                ir_hits,
                self.effect_callback_fallbacks,
                trips,
            );
        }

        // 3. Resume engines that are ready. An engine is parked in `job.engine` between steps;
        // taking it hands ownership to the next worker step.
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
                spawn_engine_step(self.thread_pool.as_ref(), goals, res_tx, engine);
            }
        }

        // 4. Clean up finished jobs that have had one frame to be inspected. Their
        // engines are freed off the main thread (see `drop_in_background`).
        let mut finished: Vec<PlannerEngine> = Vec::new();
        self.active_jobs.retain_mut(|job| {
            if job.pending_reap {
                finished.extend(job.engine.take());
                false
            } else {
                true
            }
        });
        drop_in_background(finished);
        for job in self.active_jobs.iter_mut().filter(|j| j.done) {
            job.pending_reap = true;
        }

        crate::logger::process_logs();
    }

    /// Submits a planning job for `agent`, cancelling its unfinished one. Blackboards are
    /// snapshotted now. Already-satisfied goals are dropped; the rest are tried by descending
    /// reward and the first with a plan wins. Budgets are clamped: `max_recursion` >= 1,
    /// `iteration_budget` >= 100, `time_slice_ms` = 0 disables time-slicing,
    /// `max_effect_fallbacks` >= 0 (per-search GDScript effect cap).
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
        max_effect_fallbacks: i64,
    ) {
        let agent_instance_id = agent.instance_id().to_i64();

        // A newer submission supersedes the agent's earlier plans.
        for job in self
            .active_jobs
            .iter_mut()
            .filter(|j| !j.done && j.agent_instance_id == agent_instance_id)
        {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        // Everything below up to the engine construction runs on the main thread: it reads live
        // Godot objects (blackboards, callables, goal conditions) and converts them to
        // Send-safe data.
        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());
        let mut initial_provisions = crate::requirement::extract_initial_provisions(&snap_agent);
        initial_provisions.extend(extract_provisions_from_snapshot(&snap_world));

        let mut callable_registry = Vec::new();
        let action_specs = build_action_specs(&actions, &mut callable_registry);
        let mut goal_specs = build_goal_specs(&goals, &mut callable_registry);

        // Detect the highest-reward goal already satisfied by the initial state.
        // If all goals are satisfied, the scheduler can return an empty success
        // plan instead of asking the planner to search.
        let mut satisfied_goal_index = -1i64;
        let mut best_satisfied_reward = f64::NEG_INFINITY;
        // Builtins run on the submit-time snapshots through the same
        // `evaluate_builtin` the planner searches with, so the pre-filter
        // cannot disagree with simulation on nil/empty or float tolerance.
        // Only `Custom` conditions fall back to the live `PreconditionHandler`,
        // which alone can run their GDScript callables.
        let goal_satisfied = |goal: &GoalSpec| -> bool {
            goal.desired_state.iter().all(|pre| {
                match pre.evaluate_builtin(&snap_agent, &snap_world) {
                    Some(result) => result,
                    None => PreconditionHandler::from_spec(pre, &callable_registry)
                        .is_some_and(|h| h.evaluate(&agent_bb, &world_bb)),
                }
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
        let cancel_flag = Arc::new(AtomicBool::new(false));

        let (inputs, engine_config) = engine_parts_for_submit(
            action_specs,
            snap_agent,
            snap_world,
            initial_provisions,
            req_tx,
            cancel_flag.clone(),
            self.effect_ir_hits.clone(),
            self.effect_fallback_trips.clone(),
            max_recursion,
            iteration_budget,
            time_slice_ms,
            max_effect_fallbacks,
        );
        let engine = PlannerEngine::new(inputs, engine_config);

        let job = ActiveJobHandle {
            agent,
            agent_instance_id,
            callable_registry,
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

        spawn_engine_step(self.thread_pool.as_ref(), job.goals.clone(), res_tx, engine);
        self.active_jobs.push(job);
    }

    /// Number of jobs the scheduler is still tracking, including finished ones that have not
    /// been reaped yet.
    #[func]
    fn active_job_count(&self) -> i64 {
        self.active_jobs.len() as i64
    }

    /// Cancel all in-flight planning jobs for `agent`. No result is delivered.
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

    /// Cancels and removes all jobs; engines are freed off the main thread.
    #[func]
    fn clear_active_jobs(&mut self) {
        for job in &mut self.active_jobs {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }
        let engines: Vec<PlannerEngine> = self
            .active_jobs
            .drain(..)
            .filter_map(|mut job| job.engine.take())
            .collect();
        drop_in_background(engines);
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

    /// Returns a string describing the current state of the thread pool, for diagnostics.
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

    /// Returns effect dispatch totals: native IR applications vs GDScript effect callbacks
    /// vs fallback-budget trips. Cost and precondition callbacks are served on the same
    /// channel but never counted here.
    #[func]
    fn get_effect_dispatch_stats(&self) -> VarDictionary {
        let mut stats = VarDictionary::new();
        stats.set(
            "effect_ir_hits",
            self.effect_ir_hits.load(Ordering::Relaxed) as i64,
        );
        stats.set(
            "effect_callback_fallbacks",
            self.effect_callback_fallbacks as i64,
        );
        stats.set(
            "effect_fallback_trips",
            self.effect_fallback_trips.load(Ordering::Relaxed) as i64,
        );
        stats
    }

    /// Sets log verbosity: 0 errors, 1 warnings, 2 info (default), 3 debug. Debug also records
    /// search trees for jobs started afterwards.
    /// Version of this Rust library, the same as the addon's `plugin.cfg` version in a matching
    /// build. The autoload warns when they differ (a stale or foreign binary).
    #[func]
    fn get_engine_version(&self) -> String {
        env!("CARGO_PKG_VERSION").to_string()
    }

    #[func]
    fn set_log_level(&self, level: i64) {
        let log_level = crate::logger::LogLevel::from_u8(level.clamp(0, 3) as u8);
        crate::logger::set_log_level(log_level);
    }

    /// Returns the search tree for `agent`'s job, or an empty string if none was recorded.
    #[func]
    fn get_debug_tree(&self, agent: Gd<Object>) -> String {
        let agent_instance_id = agent.instance_id().to_i64();
        // First, prefer a non-cancelled job
        for job in &self.active_jobs {
            if job.agent_instance_id == agent_instance_id
                && !job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed)
            {
                if let Some(engine) = &job.engine {
                    return engine.debug_tree().format();
                } else {
                    return "Engine is currently running in a background thread.".to_string();
                }
            }
        }
        // Fall back to any matching job (including cancelled)
        for job in &self.active_jobs {
            if job.agent_instance_id == agent_instance_id {
                if let Some(engine) = &job.engine {
                    return engine.debug_tree().format();
                } else {
                    return "Engine is currently running in a background thread.".to_string();
                }
            }
        }
        String::new()
    }
}

/// Frees `items` on a background thread and returns its handle (`None` when empty).
///
/// A finished search engine owns every open branch, each with its own agent and
/// world snapshot, so dropping a large one takes up to ~1.5 s. Doing that inline
/// in `process_callbacks` stalled the game frame. A plain thread keeps the cost off
/// both the main thread and the planning pool.
pub(crate) fn drop_in_background<T: Send + 'static>(
    items: Vec<T>,
) -> Option<std::thread::JoinHandle<()>> {
    if items.is_empty() {
        return None;
    }
    Some(std::thread::spawn(move || drop(items)))
}

/// Runs one step of `engine` on the pool and sends it back on `res_tx`. Drops the engine if the
/// pool does not exist yet.
fn spawn_engine_step(
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

/// Builds one submission's engine inputs and config from submit-time data. Budgets are
/// clamped as documented on [`GdPAIPlanScheduler::submit_plan`]; the fallback budget
/// lives only in the returned config's `max_effect_fallbacks`, so `EngineConfig`
/// stays the single source of truth.
#[allow(clippy::too_many_arguments)]
pub(crate) fn engine_parts_for_submit(
    actions: Vec<ActionSpec>,
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    initial_provisions: Vec<ProvisionSpec>,
    request_tx: Sender<CallbackRequest>,
    cancel_flag: Arc<AtomicBool>,
    effect_ir_hits: Arc<AtomicU64>,
    effect_fallback_trips: Arc<AtomicU64>,
    max_recursion: i64,
    iteration_budget: i64,
    time_slice_ms: i64,
    max_effect_fallbacks: i64,
) -> (EngineInputs, EngineConfig) {
    let max_rec = max_recursion.max(1) as usize;
    let iter_budget = iteration_budget.max(100) as usize;
    let slice_ms = time_slice_ms.max(0) as u64;
    let fallback_budget = max_effect_fallbacks.max(0) as usize;
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
    if max_effect_fallbacks < 0 {
        log_warn!(
            "max_effect_fallbacks was clamped from {} to 0",
            max_effect_fallbacks
        );
    }

    let engine_config = EngineConfig {
        max_depth: max_rec,
        iteration_budget: iter_budget,
        time_slice_ms: slice_ms,
        max_effect_fallbacks: fallback_budget,
        ..EngineConfig::default()
    };
    let inputs = EngineInputs {
        actions,
        agent,
        world,
        initial_provisions,
        request_tx,
        cancel_flag,
        effect_ir_hits,
        effect_fallback_trips,
    };
    (inputs, engine_config)
}

/// Appends `callable` to the job's registry and returns its index, which stands in for it in
/// the Send-safe specs.
fn register_callable(registry: &mut Vec<Callable>, callable: Callable) -> usize {
    let id = registry.len();
    registry.push(callable);
    id
}

/// Converts the bridge's action dictionaries into [`ActionSpec`]s. An action without a name is
/// skipped, which would misalign indices with the GDScript array; the bridge always names them.
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
            let effect_spec = parse_effect_spec(&dict, &name);
            let effect_id = if effect_spec.is_some() {
                // The native spec serves every effect: no callable to register.
                None
            } else {
                effect_val.as_ref().and_then(|v| {
                    if let Ok(c) = v.try_to::<Callable>() {
                        if c.is_valid() {
                            Some(register_callable(registry, c))
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
            };

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
                effect_spec,
                preconditions,
                validity_checks,
                requirements,
                provisions,
                dependent_object_ids,
            })
        })
        .collect()
}

/// Converts the bridge's `effect_spec` arrays into native [`EffectOp`]s. A missing or empty
/// array means "no spec" without a warning; a present-but-unparseable one warns once per action
/// and falls back to the effect callable. The bridge always sends a typed
/// `Array[Dictionary]` (see `gdpai_rust_bridge.gd`), so only that shape is accepted here.
fn parse_effect_spec(dict: &VarDictionary, action_name: &str) -> Option<Vec<EffectOp>> {
    let raw = dict.get("effect_spec")?;
    if raw.is_nil() {
        return None;
    }
    let Ok(typed) = raw.try_to::<Array<VarDictionary>>() else {
        log_warn!(
            "Action '{}': ignoring malformed effect_spec; falling back to effect_callable",
            action_name
        );
        return None;
    };
    let op_dicts: Vec<VarDictionary> = typed.iter_shared().collect();
    if op_dicts.is_empty() {
        return None;
    }
    let mut ops = Vec::with_capacity(op_dicts.len());
    for op_dict in &op_dicts {
        match EffectOp::from_dict(op_dict) {
            Some(op) => ops.push(op),
            None => {
                log_warn!(
                    "Action '{}': ignoring malformed effect_spec; falling back to effect_callable",
                    action_name
                );
                return None;
            }
        }
    }
    Some(ops)
}

/// Converts the serialized goal dictionaries into [`GoalSpec`]s. Goals without a `name` or
/// `reward` are skipped; `original_index` keeps the position in the input array.
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

/// Reads `dict[key]` as an array of dictionaries (typed or untyped) and parses each entry with
/// `parse_fn`, dropping entries it rejects. A missing key gives an empty vector.
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

/// [`extract_typed_specs`] for precondition dictionaries, registering custom callables.
fn extract_precond_specs(
    dict: &VarDictionary,
    key: &str,
    registry: &mut Vec<Callable>,
) -> Vec<PreconditionSpec> {
    extract_typed_specs(dict, key, |d| precond_spec_from_dict(d, registry))
}

/// [`extract_typed_specs`] for requirement dictionaries.
fn extract_requirement_specs(dict: &VarDictionary, key: &str) -> Vec<RequirementSpec> {
    extract_typed_specs(dict, key, RequirementSpec::from_dict)
}

/// [`extract_typed_specs`] for provision dictionaries.
fn extract_provision_specs(dict: &VarDictionary, key: &str) -> Vec<ProvisionSpec> {
    extract_typed_specs(dict, key, ProvisionSpec::from_dict)
}

/// Builds a [`PreconditionSpec`], registering a custom precondition's callable.
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

/// Runs one requested callable on fresh blackboards rebuilt from the snapshots. A non-number
/// cost means infinity; a non-bool precondition means false; effects are diffed back.
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

            let new_agent = Arc::new(BlackboardSnapshot::patch_from(&agent, &bb_agent.bind()));
            let new_world = Arc::new(BlackboardSnapshot::patch_from(&world, &bb_world.bind()));
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

/// Sets each binding as an agent property (its first value), so callables can read e.g.
/// `at_target`. Routes through `set_property` so the injected keys are marked dirty for
/// diff-back, exactly like an effect mutation.
fn inject_bindings_into_agent(
    agent_bb: &mut Gd<GdPAIBlackboard>,
    bindings: &[(String, Vec<VariantSnapshot>)],
) {
    let mut bind = agent_bb.bind_mut();
    for (name, values) in bindings {
        if !values.is_empty() {
            bind.set_property(GString::from(name.as_str()), values[0].to_variant());
        }
    }
}

/// Turns each world object's `provides` property into `Fact(provides, [object id])`.
fn extract_provisions_from_snapshot(snap: &BlackboardSnapshot) -> Vec<ProvisionSpec> {
    let mut provisions = Vec::new();
    for (uid, obj) in snap.objects.iter() {
        for (prop_name, val) in obj.properties.iter() {
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

/// Converts a [`PlanResult`] into the dictionary delivered to `_on_plan_ready`.
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

// The scheduler is a Godot class: these test the helpers behind it, which Rust tests cannot
// reach through Godot.
#[cfg(test)]
mod tests {
    use super::{StallNoticeThrottle, drop_in_background, engine_parts_for_submit};
    use crate::planner::{EngineConfig, PlannerEngine};
    use crate::snapshot::BlackboardSnapshot;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

    /// submit_plan's wiring lives in `engine_parts_for_submit`, which this pins: the
    /// engine carries the configured fallback budget into its search context.
    /// submit_plan itself needs live Godot blackboards and callables, so it cannot
    /// run in a Rust unit test.
    #[test]
    fn submit_wiring_threads_configured_fallback_budget() {
        let empty_snapshot = || BlackboardSnapshot {
            properties: Arc::new(HashMap::new()),
            objects: Arc::new(HashMap::new()),
        };
        let (req_tx, _req_rx) = std::sync::mpsc::channel();
        let (inputs, config) = engine_parts_for_submit(
            Vec::new(),
            empty_snapshot(),
            empty_snapshot(),
            Vec::new(),
            req_tx,
            Arc::default(),
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
            5,
            1000,
            10,
            7,
        );
        assert_eq!(
            config.max_effect_fallbacks, 7,
            "custom budget flows through the wiring"
        );
        let engine = PlannerEngine::new(inputs, config);
        assert_eq!(
            engine.ctx.effect_fallback_budget, 7,
            "the configured value reaches the search context"
        );

        let (req_tx, _req_rx) = std::sync::mpsc::channel();
        let (inputs, config) = engine_parts_for_submit(
            Vec::new(),
            empty_snapshot(),
            empty_snapshot(),
            Vec::new(),
            req_tx,
            Arc::default(),
            Arc::new(AtomicU64::new(0)),
            Arc::new(AtomicU64::new(0)),
            5,
            1000,
            10,
            -5,
        );
        assert_eq!(
            config.max_effect_fallbacks, 0,
            "negative budget clamps to 0 in the config"
        );
        let engine = PlannerEngine::new(inputs, config);
        assert_eq!(
            engine.ctx.effect_fallback_budget, 0,
            "the clamped value reaches the search context"
        );
        assert_eq!(
            EngineConfig::default().max_effect_fallbacks,
            128,
            "unset budget defaults to 128"
        );
    }

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

    /// Reports the thread it is dropped on.
    struct DropProbe(std::sync::mpsc::Sender<std::thread::ThreadId>);

    impl Drop for DropProbe {
        fn drop(&mut self) {
            let _ = self.0.send(std::thread::current().id());
        }
    }

    /// Finished engines are freed off the calling (main) thread.
    #[test]
    fn drops_items_on_another_thread() {
        let (tx, rx) = std::sync::mpsc::channel();
        let handle = drop_in_background(vec![DropProbe(tx.clone()), DropProbe(tx)])
            .expect("non-empty input spawns a thread");
        handle.join().unwrap();
        let caller = std::thread::current().id();
        let dropped_on: Vec<std::thread::ThreadId> = rx.try_iter().collect();
        assert_eq!(dropped_on.len(), 2, "every item is dropped");
        assert!(
            dropped_on.iter().all(|id| *id != caller),
            "never on the caller's thread"
        );
    }

    #[test]
    fn empty_input_spawns_nothing() {
        assert!(drop_in_background(Vec::<u8>::new()).is_none());
    }
}
