//! Regression test for a potential infinite loop in the planner's Verifying state.

mod common;
use common::{inputs, run_to_completion, spawn_responder};

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackResponse, GoalSpec, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine, TerminationStrategy};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;

#[test]
fn test_verifying_terminal_state() {
    let mut props = HashMap::new();
    props.insert("goal_met".to_string(), VariantSnapshot::Bool(false));
    let agent = BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    // Action that meets the goal
    let actions = vec![ActionSpec {
        name: "finish".to_string(),
        cost_callable_id: None,
        effect_callable_id: Some(1),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "goal".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    // Mock responder that makes the action satisfy the goal
    let req_tx = spawn_responder(|req| match req.kind {
        CallbackKind::ApplyEffect { mut agent, world } => {
            agent
                .properties
                .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
            CallbackResponse::UpdatedSnapshots(agent, world)
        }
        _ => CallbackResponse::Float(1.0),
    });

    let mut engine = PlannerEngine::new(
        inputs(actions, agent, world, req_tx),
        EngineConfig {
            max_depth: 10,
            termination: TerminationStrategy::FirstComplete,
            ..EngineConfig::default()
        },
    );
    let plan = run_to_completion(&mut engine, &goals).expect("Should have found a plan");
    assert!(plan.success, "Plan should be successful");
}
