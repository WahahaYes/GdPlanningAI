//! Shared helpers for the integration tests: snapshot builders, a stand-in for the Godot main
//! thread, and a loop that runs an engine to completion.

// Each test file compiles this module on its own and uses only part of it.
#![allow(dead_code)]

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult,
};
use gdplanningai_rust::planner::{EngineInputs, PlannerEngine};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, SimObjectData, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::{self, Sender};
use std::thread;
use std::time::{Duration, Instant};

/// Create a test agent snapshot with basic properties.
pub fn create_test_agent(properties: Vec<(&str, VariantSnapshot)>) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

/// Create a test world snapshot with objects.
pub fn create_test_world(
    properties: Vec<(&str, VariantSnapshot)>,
    objects: Vec<(&str, SimObjectData)>,
) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }

    let mut objs = HashMap::new();
    for (uid, obj) in objects {
        objs.insert(uid.to_string(), obj);
    }

    BlackboardSnapshot {
        properties: props,
        objects: objs,
    }
}

/// Create a simple SimObjectData for testing.
pub fn create_sim_object(
    uid: &str,
    groups: Vec<&str>,
    properties: Vec<(&str, VariantSnapshot)>,
) -> SimObjectData {
    let groups_vec = groups.iter().map(|s| s.to_string()).collect();
    let mut props = HashMap::new();
    for (key, value) in properties {
        props.insert(key.to_string(), value);
    }

    SimObjectData {
        uid: uid.to_string(),
        groups: groups_vec,
        properties: props,
    }
}

/// Engine inputs with no initial provisions and a fresh cancel flag.
pub fn inputs(
    actions: Vec<ActionSpec>,
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    request_tx: Sender<CallbackRequest>,
) -> EngineInputs {
    EngineInputs {
        actions,
        agent,
        world,
        initial_provisions: vec![],
        request_tx,
        cancel_flag: Arc::default(),
        effect_ir_hits: Arc::new(AtomicU64::new(0)),
    }
}

/// Stands in for the Godot main thread: answers every callback request with `answer(request)`
/// on its own thread, until the engine holding the returned sender is dropped.
pub fn spawn_responder(
    mut answer: impl FnMut(CallbackRequest) -> CallbackResponse + Send + 'static,
) -> Sender<CallbackRequest> {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let request_id = req.request_id;
            let response_tx = req.response_tx.clone();
            let response = answer(req);
            let _ = response_tx.send(PlannerCallback {
                request_id,
                response,
            });
        }
    });
    req_tx
}

/// Lowers the agent's int `hunger` by `by`, not below 0; no-op if it has none.
pub fn reduce_hunger(agent: &mut BlackboardSnapshot, by: i64) {
    if let Some(VariantSnapshot::Int(current)) = agent.properties.get("hunger").cloned() {
        agent.properties.insert(
            "hunger".to_string(),
            VariantSnapshot::Int((current - by).max(0)),
        );
    }
}

/// Answers for [`spawn_responder`]: every action costs `cost`, every effect lowers hunger by
/// `hunger_reduction`, and custom preconditions hold.
pub fn hunger_answers(
    cost: f64,
    hunger_reduction: i64,
) -> impl FnMut(CallbackRequest) -> CallbackResponse + Send + 'static {
    move |req| match req.kind {
        CallbackKind::GetCost { .. } => CallbackResponse::Float(cost),
        CallbackKind::ApplyEffect { mut agent, world } => {
            reduce_hunger(&mut agent, hunger_reduction);
            CallbackResponse::UpdatedSnapshots(agent, world)
        }
        CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(true),
    }
}

/// Steps `engine` until it completes, sleeping 1 ms between steps so answers can arrive.
/// Panics after 15 s.
pub fn run_to_completion(engine: &mut PlannerEngine, goals: &[GoalSpec]) -> Option<PlanResult> {
    run_counting_yields(engine, goals).0
}

/// [`run_to_completion`], also counting the `Pending(0)` budget and time-slice yields.
pub fn run_counting_yields(
    engine: &mut PlannerEngine,
    goals: &[GoalSpec],
) -> (Option<PlanResult>, usize) {
    let start = Instant::now();
    let mut yields = 0;
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(15),
            "timed out waiting for plan completion"
        );
        match engine.plan(goals) {
            PlannerRunResult::Complete(res) => return (res, yields),
            PlannerRunResult::Pending(id) => {
                if id == 0 {
                    yields += 1;
                }
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
