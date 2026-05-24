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
use crate::planner::{PlannerEngine, SearchContext, SearchAlgorithm, TerminationStrategy};
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
    cancel_flag: Arc<AtomicBool>,
    pending_request_id: usize,
    done: bool,
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
                match run_result {
                    PlannerRunResult::Complete(result) => {
                        job.done = true;
                        if job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) {
                            continue;
                        }

                        let Some(result) = result else {
                            // If search exhausted with no plan, deliver a failed result
                            if job.agent.is_instance_valid() {
                                let failed_res = PlanResult {
                                    success: false,
                                    action_chain: vec![],
                                    total_cost: 0.0,
                                    goal_index: -1,
                                    deferred_action_indices: vec![],
                                    action_bindings: vec![],
                                };
                                let dict = result_to_dict(&failed_res);
                                job.agent.call("_on_plan_ready", &[dict.to_variant()]);
                            }
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
                            job.agent.call("_on_plan_ready", &[dict.to_variant()]);
                        }
                    }
                    PlannerRunResult::Pending(id) => {
                        job.engine = Some(engine);
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
                let response = dispatch_callback(callable, req.kind);
                
                let _ = req.response_tx.send(PlannerCallback {
                    request_id: req.request_id,
                    response,
                });
                jobs_with_responses.insert(job.agent_instance_id);
            }
        }

        // 3. Resume engines that are ready
        for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
            let ready_to_resume = if job.pending_request_id == 0 {
                // Yielded for budget, always ready
                true
            } else {
                // Yielded for callback, only ready if we got a response
                jobs_with_responses.contains(&job.agent_instance_id)
            };

            if ready_to_resume {
                if let Some(engine) = job.engine.take() {
                    let goals = job.goals.clone();
                    let res_tx = job.result_tx.clone();
                    run_job_step(self.thread_pool.as_ref(), goals, res_tx, engine);
                }
            }
        }

        // Clean up finished jobs
        self.active_jobs.retain(|job| !job.done);
        
        crate::logger::process_logs();
    }

    /// Submit a planning job for `agent`.
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

        for job in self.active_jobs.iter_mut().filter(|j| !j.done && j.agent_instance_id == agent_instance_id) {
            job.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        let snap_agent = BlackboardSnapshot::from_blackboard(&agent_bb.bind());
        let snap_world = BlackboardSnapshot::from_blackboard(&world_bb.bind());
        let mut initial_provisions = crate::requirement::extract_initial_provisions(&snap_agent);
        initial_provisions.extend(extract_provisions_from_snapshot(&snap_world));

        let mut job_registry = Vec::new();
        let action_specs = build_action_specs(&actions, &mut job_registry);
        let goal_specs = build_goal_specs(&goals, &mut job_registry);

        let (req_tx, req_rx) = std::sync::mpsc::channel::<CallbackRequest>();
        let (res_tx, res_rx) = std::sync::mpsc::channel::<(PlannerRunResult, PlannerEngine)>();
        let (engine_tx, engine_rx) = std::sync::mpsc::channel::<PlannerCallback>();
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let max_rec = max_recursion.max(1) as usize;

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
        });

        let mut engine = PlannerEngine::new(ctx, max_rec, cancel_flag.clone())
            .with_search_algorithm(SearchAlgorithm::AStar)
            .with_termination_strategy(TerminationStrategy::BestCost);
        
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
            cancel_flag,
            pending_request_id: 0,
            done: false,
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
                job.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
                log_debug!("Cancelled planning job for agent instance {}", job.agent_instance_id);
            }
        }
    }

    /// Clear all active jobs from the scheduler.
    #[func]
    fn clear_active_jobs(&mut self) {
        for job in &mut self.active_jobs {
            job.cancel_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
        self.active_jobs.clear();
        log_debug!("Cleared all active jobs from scheduler");
    }

    /// Sets the process-wide log verbosity.
    #[func]
    fn set_log_level(&self, level: i64) {
        let log_level = crate::logger::LogLevel::from_u8(level.clamp(0, 3) as u8);
        crate::logger::set_log_level(log_level);
    }
}

fn run_job_step(thread_pool: Option<&rayon::ThreadPool>, goals: Vec<GoalSpec>, res_tx: Sender<(PlannerRunResult, PlannerEngine)>, engine: PlannerEngine) {
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
            let name = dict.get("name")?.try_to::<String>().ok()?;
            
            let cost_val = dict.get("cost_callable");
            let cost_id = cost_val.as_ref()
                .and_then(|v| {
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
            let effect_id = effect_val.as_ref()
                .and_then(|v| {
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
    match kind {
        CallbackKind::GetCost { agent, world, provisions, bindings } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            let prov_arr = provisions_to_array(&provisions);
            let bind_dict = bindings_to_dict(&bindings);
            
            {
                let mut bind = bb_agent.bind_mut();
                for (name, values) in &bindings {
                    if !values.is_empty() {
                        bind.properties.insert(name.clone(), values[0].to_variant());
                    }
                }
            }

            let mut args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            let expected_count = callable.get_argument_count();
            if expected_count >= 3 {
                args.push(prov_arr.to_variant());
            }
            if expected_count >= 4 {
                args.push(bind_dict.to_variant());
            }

            let result = callable.call(&args);
            let cost = if let Ok(f) = result.try_to::<f64>() {
                f
            } else if let Ok(i) = result.try_to::<i64>() {
                i as f64
            } else {
                f64::INFINITY
            };
            CallbackResponse::Float(cost)
        }
        CallbackKind::ApplyEffect { agent, world, provisions, bindings } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            let prov_arr = provisions_to_array(&provisions);
            let bind_dict = bindings_to_dict(&bindings);

            {
                let mut bind = bb_agent.bind_mut();
                for (name, values) in &bindings {
                    if !values.is_empty() {
                        bind.properties.insert(name.clone(), values[0].to_variant());
                    }
                }
            }

            let mut args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            let expected_count = callable.get_argument_count();
            if expected_count >= 3 {
                args.push(prov_arr.to_variant());
            }
            if expected_count >= 4 {
                args.push(bind_dict.to_variant());
            }

            callable.call(&args);
            
            let new_agent = BlackboardSnapshot::from_blackboard(&bb_agent.bind());
            let new_world = BlackboardSnapshot::from_blackboard(&bb_world.bind());
            CallbackResponse::UpdatedSnapshots(new_agent, new_world)
        }
        CallbackKind::EvalCustomPrecond { agent, world, provisions, bindings } => {
            let mut bb_agent = agent.into_blackboard();
            let bb_world = world.into_blackboard();
            let prov_arr = provisions_to_array(&provisions);
            let bind_dict = bindings_to_dict(&bindings);

            {
                let mut bind = bb_agent.bind_mut();
                for (name, values) in &bindings {
                    if !values.is_empty() {
                        bind.properties.insert(name.clone(), values[0].to_variant());
                    }
                }
            }

            let mut args = vec![bb_agent.to_variant(), bb_world.to_variant()];
            let expected_count = callable.get_argument_count();
            if expected_count >= 3 {
                args.push(prov_arr.to_variant());
            }
            if expected_count >= 4 {
                args.push(bind_dict.to_variant());
            }

            let result = callable.call(&args);
            CallbackResponse::Bool(result.try_to::<bool>().unwrap_or(false))
        }
    }
}

fn provisions_to_array(provisions: &[ProvisionSpec]) -> Array<VarDictionary> {
    let mut arr = Array::<VarDictionary>::new();
    for prov in provisions {
        let mut dict = VarDictionary::new();
        match prov {
            ProvisionSpec::Binding { binding_name, value } => {
                dict.set("kind", "binding");
                dict.set("binding_name", binding_name.clone());
                dict.set("value", value.to_variant());
            }
            ProvisionSpec::Fact { fact_name, args } => {
                dict.set("kind", "fact");
                dict.set("fact_name", fact_name.clone());
                let mut args_arr = Array::<Variant>::new();
                for arg in args {
                    args_arr.push(&arg.to_variant());
                }
                dict.set("args", args_arr.to_variant());
            }
            ProvisionSpec::FactWildcard { fact_name } => {
                dict.set("kind", "fact_wildcard");
                dict.set("fact_name", fact_name.clone());
            }
        }
        arr.push(&dict);
    }
    arr
}

fn bindings_to_dict(bindings: &[(String, Vec<crate::snapshot::VariantSnapshot>)]) -> VarDictionary {
    let mut dict = VarDictionary::new();
    for (name, values) in bindings {
        let mut vals_arr = Array::<Variant>::new();
        for val in values {
            vals_arr.push(&val.to_variant());
        }
        dict.set(name.clone(), vals_arr.to_variant());
    }
    dict
}

fn extract_provisions_from_snapshot(snap: &BlackboardSnapshot) -> Vec<ProvisionSpec> {
    let mut provisions = Vec::new();
    for (uid, obj) in &snap.objects {
        for (prop_name, val) in &obj.properties {
            if prop_name == "provides" {
                if let VariantSnapshot::Str(fact_name) = val {
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
