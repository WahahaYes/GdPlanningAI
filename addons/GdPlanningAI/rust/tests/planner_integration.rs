//! Engine behavior the scenario fixtures (`scenarios.rs`) cannot express: how many effect
//! callbacks a plan costs. Result-shaped planner cases belong in `tests/scenarios/`.

mod common;
use common::{hunger_answers, inputs, run_to_completion, spawn_responder};

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, GoalSpec, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::mpsc;

fn make_agent(hunger: i64) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(hunger));
    BlackboardSnapshot {
        properties: Arc::new(props),
        objects: Arc::new(HashMap::new()),
    }
}

fn make_world() -> BlackboardSnapshot {
    BlackboardSnapshot {
        properties: Arc::new(HashMap::new()),
        objects: Arc::new(HashMap::new()),
    }
}

fn hunger_less_than(threshold: i64) -> PreconditionSpec {
    PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::LessThan,
        property_name: "hunger".to_string(),
        value: Some(VariantSnapshot::Int(threshold)),
    }
}

fn run_planner(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    max_depth: usize,
    request_tx: mpsc::Sender<CallbackRequest>,
) -> Option<PlanResult> {
    gdplanningai_rust::logger::init_log_channel();
    gdplanningai_rust::logger::set_log_level(gdplanningai_rust::logger::LogLevel::Debug);

    let mut engine = PlannerEngine::new(
        inputs(actions, agent, world, request_tx),
        EngineConfig {
            max_depth,
            ..EngineConfig::default()
        },
    );
    run_to_completion(&mut engine, &goals)
}

#[test]
fn verification_reuses_the_discovered_first_step() {
    // Discovery already simulated eat from the initial state, so verifying the one-step plan
    // reuses that post-state: the effect callback fires once, not twice.
    let actions = vec![ActionSpec {
        name: "eat".to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(0),
        effect_spec: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];
    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let effect_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = Arc::clone(&effect_calls);
    let mut answer = hunger_answers(5.0, 60);
    let req_tx = spawn_responder(move |req| {
        if matches!(req.kind, CallbackKind::ApplyEffect { .. }) {
            counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        }
        answer(req)
    });

    let plan = run_planner(make_agent(80), make_world(), actions, goals, 10, req_tx)
        .expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain, vec![0]);
    assert_eq!(
        effect_calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "verification should reuse discovery's simulation of step 0"
    );
}
