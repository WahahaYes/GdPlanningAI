//! Plan scheduler exposed to GDScript.
//!
//! [`GdPAIPlanScheduler`] owns a Rayon thread pool and a callable registry.
//! GDScript agents submit planning jobs via [`submit_plan`]; each frame the
//! autoload calls [`process_callbacks`] to drain pending callback requests
//! from planner threads and deliver completed results.

use crate::plan_tree::PlanResult;
use crate::plan_types::*;
use crate::precondition::{PreconditionHandler, PreconditionOp};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use godot::prelude::*;
use std::sync::mpsc::Receiver;
use std::sync::{Arc, atomic::AtomicBool};

struct ActiveJobHandle {
    agent: Gd<Object>,
    agent_instance_id: i64,
    callable_registry: Vec<Callable>,
    request_rx: Receiver<CallbackRequest>,
    result_rx: Receiver<Option<PlanResult>>,
    cancel_flag: Arc<AtomicBool>,
    done: bool,
}

/// Planning scheduler.
///
/// Add as a child of the autoload and call [method process_callbacks] every
/// frame. Agents submit jobs with [method submit_plan]; results arrive via
/// `_on_plan_ready(result: Dictionary)` called on the agent.
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
        crate::logger::init_log_channel();

        let mut builder = rayon::ThreadPoolBuilder::new();
        if self.max_threads > 0 {
            builder = builder.num_threads(self.max_threads as usize);
        }
        self.thread_pool = Some(
            builder
                .build()
                .expect("GdPAIPlanScheduler: failed to build Rayon thread pool"),
        );
        godot::prelude::godot_print!("[GdPAI] Direct print from scheduler ready - logging works");
        let num_threads = self
            .thread_pool
            .as_ref()
            .map(|tp| tp.current_num_threads())
            .unwrap_or(0);
        log_info!(
            "GdPAIPlanScheduler ready — {} worker thread(s)",
            num_threads
        );
        log_debug!("Log channel initialized and ready for planner thread logging");
    }
}

#[godot_api]
impl GdPAIPlanScheduler {
    /// Drain pending callback requests and deliver completed results.
    /// Call this once per frame from GDScript `_process`.
    #[func]
    fn process_callbacks(&mut self) {
        // Process pending log messages from planner threads (from previous frames)
        crate::logger::process_logs();

        // let active_count = self.active_jobs.iter().filter(|j| !j.done).count();
        // let total_count = self.active_jobs.len();

        // Process each job's pending callbacks using its own callable registry.
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            while let Ok(req) = job.request_rx.try_recv() {
                let callable = &job.callable_registry[req.callable_id];
                let response = dispatch_callback(callable, req.kind);
                let _ = req.response_tx.send(response);
            }
            // Check for completed plan
            if let Ok(result) = job.result_rx.try_recv() {
                job.done = true;
                if job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) || result.is_none() {
                    log_debug!("Canceled plan job finished without delivery");
                    continue;
                }

                let Some(result) = result else {
                    log_warn!("Plan job returned None result");
                    continue;
                };

                if job.agent.is_instance_valid() {
                    log_info!(
                        "Plan complete: success={}, actions={}, cost={:.1}",
                        result.success,
                        result.action_chain.len(),
                        result.total_cost
                    );
                    let dict = result_to_dict(&result);
                    job.agent
                        .clone()
                        .call("_on_plan_ready", &[dict.to_variant()]);
                } else {
                    log_warn!("Plan completed but agent was freed");
                }
            }
        }
        // Clean up finished jobs — Rayon owns the threads, no join needed.
        self.active_jobs.retain(|job| !job.done);

        // Drain any log messages generated during this callback processing
        crate::logger::process_logs();
    }

    /// Submit a planning job for `agent`.
    ///
    /// `agent_bb` and `world_bb` are snapshotted immediately. `actions` and
    /// `goals` are `Array[Dictionary]` serialised by [GdPAIRustBridge].
    /// `max_recursion` caps the planner search depth for this job.
    /// When the plan is ready, `agent._on_plan_ready(result)` is called.
    #[func]
    fn submit_plan(
        &mut self,
        agent: Gd<Object>,
        agent_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        world_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        actions: Array<VarDictionary>,
        goals: Array<VarDictionary>,
        max_recursion: i64,
    ) {
        let agent_instance_id = agent.instance_id().to_i64();

        for job in self
            .active_jobs
            .iter_mut()
            .filter(|job| !job.done && job.agent_instance_id == agent_instance_id)
        {
            job.cancel_flag
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        // 1. Snapshot blackboards on main thread
        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());

        // 2. Register callables and build specs (per-job registry)
        let mut job_registry = Vec::new();
        let action_specs = build_action_specs(&actions, &mut job_registry);
        let goal_specs = build_goal_specs(&goals, &mut job_registry);

        log_info!(
            "Submitting plan: {} actions, {} goals, {} callables",
            action_specs.len(),
            goal_specs.len(),
            job_registry.len()
        );

        // 3. Create channels
        let (req_tx, req_rx) = std::sync::mpsc::channel::<CallbackRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<Option<PlanResult>>();
        let cancel_flag = Arc::new(AtomicBool::new(false));

        // 4. Dispatch to Rayon
        let max_rec = max_recursion.max(1) as usize;
        let worker_cancel_flag = cancel_flag.clone();
        self.thread_pool
            .as_ref()
            .expect("submit_plan called before ready()")
            .spawn(move || {
                crate::planner::run_plan(
                    snap_agent,
                    snap_world,
                    action_specs,
                    goal_specs,
                    max_rec,
                    req_tx,
                    res_tx,
                    worker_cancel_flag,
                );
            });

        self.active_jobs.push(ActiveJobHandle {
            agent,
            agent_instance_id,
            callable_registry: job_registry,
            request_rx: req_rx,
            result_rx: res_rx,
            cancel_flag,
            done: false,
        });
    }

    /// Number of jobs currently in flight.
    #[func]
    fn active_job_count(&self) -> i64 {
        self.active_jobs.len() as i64
    }

    /// Sets the process-wide log verbosity.
    ///
    /// [param level]: [code]0[/code] = Error, [code]1[/code] = Warn,
    /// [code]2[/code] = Info (default), [code]3[/code] = Debug.
    #[func]
    fn set_log_level(&self, level: i64) {
        let log_level = crate::logger::LogLevel::from_u8(level.clamp(0, 3) as u8);
        crate::logger::set_log_level(log_level);
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
            let name = dict.get("name")?.try_to::<String>().ok()?;
            let cost_callable = dict.get("cost_callable")?.try_to::<Callable>().ok()?;
            let effect_callable = dict.get("effect_callable")?.try_to::<Callable>().ok()?;

            let cost_id = register_callable(registry, cost_callable);
            let effect_id = register_callable(registry, effect_callable);

            let preconditions = extract_precond_specs(&dict, "preconditions", registry);
            let validity_checks = extract_precond_specs(&dict, "validity_checks", registry);
            let requirements = extract_requirement_specs(&dict, "requirements");
            let provisions = extract_provision_specs(&dict, "provisions");

            // Collect all dependent object IDs from preconditions and action-level deps
            let mut dependent_object_ids: Vec<i64> = Vec::new();

            // Collect from preconditions
            for precond in &preconditions {
                dependent_object_ids.extend_from_slice(precond.dependent_object_ids());
            }
            for check in &validity_checks {
                dependent_object_ids.extend_from_slice(check.dependent_object_ids());
            }

            // Extract action-level dependent objects if present
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
            let name = dict.get("name")?.try_to::<String>().ok()?;
            let reward = dict.get("reward")?.try_to::<f64>().ok()?;
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

fn extract_precond_specs(
    dict: &VarDictionary,
    key: &str,
    registry: &mut Vec<Callable>,
) -> Vec<PreconditionSpec> {
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
        .map(|arr| {
            arr.iter_shared()
                .filter_map(|d| precond_spec_from_dict(&d, registry))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_requirement_specs(dict: &VarDictionary, key: &str) -> Vec<RequirementSpec> {
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
        .map(|arr| {
            arr.iter_shared()
                .filter_map(|d| RequirementSpec::from_dict(&d))
                .collect()
        })
        .unwrap_or_default()
}

fn extract_provision_specs(dict: &VarDictionary, key: &str) -> Vec<ProvisionSpec> {
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
        .map(|arr| {
            arr.iter_shared()
                .filter_map(|d| ProvisionSpec::from_dict(&d))
                .collect()
        })
        .unwrap_or_default()
}

fn precond_spec_from_dict(
    dict: &VarDictionary,
    registry: &mut Vec<Callable>,
) -> Option<PreconditionSpec> {
    let handler = PreconditionHandler::from_dict(dict)?;

    if handler.operation == PreconditionOp::CustomCallback {
        let callable = handler.eval_callable?;
        let id = register_callable(registry, callable);

        // Extract dependent object IDs if present (for validity checking)
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

fn dispatch_callback(callable: &Callable, kind: CallbackKind) -> CallbackResponse {
    // Check if callable is still valid (target object may have been freed)
    if !callable.is_valid() {
        log_warn!("Callable is no longer valid (target object freed); returning safe default");
        return match kind {
            CallbackKind::GetCost { .. } => CallbackResponse::Float(f64::INFINITY),
            CallbackKind::ApplyEffect { agent, world } => {
                // Return unchanged snapshots
                CallbackResponse::UpdatedSnapshots(agent, world)
            }
            CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(false),
        };
    }

    match kind {
        CallbackKind::GetCost { agent, world } => {
            let bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            let result = callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
            let cost = result.try_to::<f64>().unwrap_or(f64::INFINITY);
            CallbackResponse::Float(cost)
        }
        CallbackKind::ApplyEffect { agent, world } => {
            let bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
            // Re-snapshot the (now mutated) blackboards
            let new_agent = BlackboardSnapshot::from_blackboard(&bb_agent.bind());
            let new_world = BlackboardSnapshot::from_blackboard(&bb_world.bind());
            CallbackResponse::UpdatedSnapshots(new_agent, new_world)
        }
        CallbackKind::EvalCustomPrecond { agent, world } => {
            let bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            let result = callable.call(&[bb_agent.to_variant(), bb_world.to_variant()]);
            CallbackResponse::Bool(result.try_to::<bool>().unwrap_or(false))
        }
    }
}

/// Converts a [`PlanResult`] to a [`VarDictionary`] for GDScript serialization.
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
    for (chain_position, fact_name, object_ids) in &result.action_bindings {
        let mut binding_tuple = Array::<Variant>::new();
        binding_tuple.push(&chain_position.to_variant());
        binding_tuple.push(&fact_name.to_variant());
        let mut ids_array = Array::<Variant>::new();
        for id in object_ids {
            ids_array.push(&id.to_variant());
        }
        binding_tuple.push(&ids_array.to_variant());
        action_bindings.push(&binding_tuple.to_variant());
    }
    dict.set("action_bindings", action_bindings.to_variant());

    dict
}
