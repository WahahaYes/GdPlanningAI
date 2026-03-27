//! Background plan scheduler exposed to GDScript.
//!
//! [`GdPAIPlanScheduler`] owns a Rayon thread pool and a callable registry.
//! GDScript agents submit planning jobs via [`submit_plan`]; each frame the
//! autoload calls [`process_callbacks`] to drain pending callback requests
//! from background threads and deliver completed results.

use crate::background_types::*;
use crate::plan_tree::PlanResult;
use crate::precondition::{PreconditionOp, PreconditionTarget};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use godot::prelude::*;
use std::sync::mpsc::Receiver;

// ---------------------------------------------------------------------------
// ActiveJobHandle
// ---------------------------------------------------------------------------

struct ActiveJobHandle {
    agent: Gd<Object>,
    request_rx: Receiver<CallbackRequest>,
    result_rx: Receiver<PlanResult>,
    done: bool,
}

// ---------------------------------------------------------------------------
// GdPAIPlanScheduler
// ---------------------------------------------------------------------------

/// Background planning scheduler.
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
    /// Maximum search depth forwarded to the background planner.
    #[export]
    max_recursion: i64,
    callable_registry: Vec<Callable>,
    active_jobs: Vec<ActiveJobHandle>,
    thread_pool: Option<rayon::ThreadPool>,
    base: Base<Node>,
}

#[godot_api]
impl INode for GdPAIPlanScheduler {
    fn init(base: Base<Node>) -> Self {
        Self {
            max_threads: 0,
            max_recursion: 100,
            callable_registry: Vec::new(),
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
        log_info!(
            "GdPAIPlanScheduler ready — {} worker thread(s)",
            self.thread_pool.as_ref().unwrap().current_num_threads()
        );
    }
}

#[godot_api]
impl GdPAIPlanScheduler {
    /// Drain pending callback requests and deliver completed results.
    /// Call this once per frame from GDScript `_process`.
    #[func]
    fn process_callbacks(&mut self) {
        // We need to borrow `callable_registry` while iterating `active_jobs`.
        // Process each job's pending callbacks.
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            loop {
                match job.request_rx.try_recv() {
                    Ok(req) => {
                        let callable = &self.callable_registry[req.callable_id];
                        let response = dispatch_callback(callable, req.kind);
                        let _ = req.response_tx.send(response);
                    }
                    Err(_) => break,
                }
            }
            // Check for completed plan
            if let Ok(result) = job.result_rx.try_recv() {
                job.done = true;
                if job.agent.is_instance_valid() {
                    let dict = result_to_dict(&result);
                    job.agent
                        .clone()
                        .call("_on_plan_ready", &[dict.to_variant()]);
                }
            }
        }
        // Clean up finished jobs — Rayon owns the threads, no join needed.
        self.active_jobs.retain(|job| !job.done);
    }

    /// Submit a planning job for `agent`.
    ///
    /// `agent_bb` and `world_bb` are snapshotted immediately. `actions` and
    /// `goals` use the same `Array[Dictionary]` format as `build_plan`.
    /// When the plan is ready, `agent._on_plan_ready(result)` is called.
    #[func]
    fn submit_plan(
        &mut self,
        agent: Gd<Object>,
        agent_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        world_bb: Gd<crate::gdpai_blackboard::GdPAIBlackboard>,
        actions: Array<VarDictionary>,
        goals: Array<VarDictionary>,
    ) {
        // 1. Snapshot blackboards on main thread
        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());

        // 2. Register callables and build specs
        let action_specs = self.build_action_specs(&actions);
        let goal_specs = self.build_goal_specs(&goals);

        // 3. Create channels
        let (req_tx, req_rx) = std::sync::mpsc::channel::<CallbackRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<PlanResult>();

        // 4. Dispatch to Rayon
        let max_rec = self.max_recursion as usize;
        self.thread_pool
            .as_ref()
            .expect("submit_plan called before ready()")
            .spawn(move || {
                crate::background_plan::run_plan(
                    snap_agent,
                    snap_world,
                    action_specs,
                    goal_specs,
                    max_rec,
                    req_tx,
                    res_tx,
                );
            });

        self.active_jobs.push(ActiveJobHandle {
            agent,
            request_rx: req_rx,
            result_rx: res_rx,
            done: false,
        });
    }

    /// Number of jobs currently in flight.
    #[func]
    fn active_job_count(&self) -> i64 {
        self.active_jobs.len() as i64
    }
}

// ---------------------------------------------------------------------------
// Private helpers
// ---------------------------------------------------------------------------

impl GdPAIPlanScheduler {
    fn register_callable(&mut self, callable: Callable) -> usize {
        let id = self.callable_registry.len();
        self.callable_registry.push(callable);
        id
    }

    fn build_action_specs(&mut self, actions: &Array<VarDictionary>) -> Vec<ActionSpec> {
        actions
            .iter_shared()
            .filter_map(|dict| {
                let name = dict.get("name")?.try_to::<String>().ok()?;
                let cost_callable = dict.get("cost_callable")?.try_to::<Callable>().ok()?;
                let effect_callable = dict.get("effect_callable")?.try_to::<Callable>().ok()?;

                let cost_id = self.register_callable(cost_callable);
                let effect_id = self.register_callable(effect_callable);

                let preconditions = self.extract_precond_specs(&dict, "preconditions");
                let validity_checks = self.extract_precond_specs(&dict, "validity_checks");

                Some(ActionSpec {
                    name,
                    cost_callable_id: cost_id,
                    effect_callable_id: effect_id,
                    preconditions,
                    validity_checks,
                })
            })
            .collect()
    }

    fn build_goal_specs(&mut self, goals: &Array<VarDictionary>) -> Vec<GoalSpec> {
        goals
            .iter_shared()
            .enumerate()
            .filter_map(|(idx, dict)| {
                let name = dict.get("name")?.try_to::<String>().ok()?;
                let reward = dict.get("reward")?.try_to::<f64>().ok()?;
                let desired_state = self.extract_precond_specs(&dict, "desired_state");
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
        &mut self,
        dict: &VarDictionary,
        key: &str,
    ) -> Vec<PreconditionSpec> {
        dict.get(key)
            .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
            .map(|arr| {
                arr.iter_shared()
                    .filter_map(|d| self.precond_spec_from_dict(&d))
                    .collect()
            })
            .unwrap_or_default()
    }

    fn precond_spec_from_dict(&mut self, dict: &VarDictionary) -> Option<PreconditionSpec> {
        let op_str = dict
            .get("operation")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_else(|| "has_property".to_string());

        if op_str.to_lowercase() == "custom_callback" {
            let callable = dict.get("eval_callable")?.try_to::<Callable>().ok()?;
            let id = self.register_callable(callable);
            return Some(PreconditionSpec::Custom { callable_id: id });
        }

        let target = dict
            .get("target")
            .and_then(|v| v.try_to::<String>().ok())
            .map(|s| match s.to_lowercase().as_str() {
                "world_state" => PreconditionTarget::WorldState,
                _ => PreconditionTarget::Agent,
            })
            .unwrap_or(PreconditionTarget::Agent);

        let operation = match op_str.to_lowercase().as_str() {
            "has_property" => PreconditionOp::HasProperty,
            "equal" => PreconditionOp::Equal,
            "not_equal" => PreconditionOp::NotEqual,
            "greater_than" => PreconditionOp::GreaterThan,
            "greater_than_or_equal" => PreconditionOp::GreaterThanOrEqual,
            "less_than" => PreconditionOp::LessThan,
            "less_than_or_equal" => PreconditionOp::LessThanOrEqual,
            _ => PreconditionOp::HasProperty,
        };

        let property_name = dict
            .get("property_name")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        let value = dict.get("value").map(|v| VariantSnapshot::from_variant(&v));

        Some(PreconditionSpec::Builtin {
            target,
            operation,
            property_name,
            value,
        })
    }
}

// ---------------------------------------------------------------------------
// Callback dispatch (main thread only)
// ---------------------------------------------------------------------------

fn dispatch_callback(callable: &Callable, kind: CallbackKind) -> CallbackResponse {
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
    dict
}
