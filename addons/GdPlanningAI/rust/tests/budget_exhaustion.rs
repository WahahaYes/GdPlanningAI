//! Regression test for search-budget exhaustion yielding Pending instead of Complete.
//!
//! Verifies that when the iteration budget is exhausted before a complete plan can be
//! found, the planner returns `PlannerRunResult::Pending` rather than `Complete`.

mod common;
use common::{inputs, reduce_hunger, run_to_completion, spawn_responder};

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackResponse, GoalSpec, PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::mpsc;

fn make_agent() -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(80));
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

fn make_world() -> BlackboardSnapshot {
    BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    }
}

#[test]
fn iteration_budget_exhaustion_returns_pending() {
    // Goal requires hunger < 30. Agent hunger is 80.
    // One action "eat" reduces hunger by 60.
    // With iteration_budget = 0, the very first node popped from the queue
    // should trigger budget exhaustion and return Pending(0).
    let agent = make_agent();
    let world = make_world();

    let actions = vec![ActionSpec {
        name: "eat".to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(0),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(30)),
        }],
        original_index: 0,
    }];

    // The request receiver is dropped: the budget runs out before any callback.
    let mut engine = PlannerEngine::new(
        inputs(actions, agent, world, mpsc::channel().0),
        EngineConfig {
            max_depth: 10,
            iteration_budget: 0,
            ..EngineConfig::default()
        },
    );

    let result = engine.plan(&goals);
    assert!(
        matches!(result, PlannerRunResult::Pending(0)),
        "Expected Pending(0) when iteration budget is exhausted, got {:?}",
        result
    );
}

#[test]
fn max_depth_prevents_plan_completion() {
    // Goal requires hunger < 30. Agent hunger is 80.
    // Action "eat" has precondition "has_food".
    // Action "get_food" satisfies "has_food".
    // With max_depth = 1, the planner can only prepend one action,
    // so it cannot find a complete 2-action plan. It should exhaust
    // the search and return a failed Complete result (not panic or hang).
    let agent = make_agent();
    let world = make_world();

    let actions = vec![
        ActionSpec {
            name: "eat".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(0),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::HasProperty,
                property_name: "has_food".to_string(),
                value: None,
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "get_food".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(1),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(30)),
        }],
        original_index: 0,
    }];

    let req_tx = spawn_responder(|req| match req.kind {
        CallbackKind::GetCost { .. } => CallbackResponse::Float(1.0),
        CallbackKind::ApplyEffect { mut agent, world } => {
            match req.callable_id {
                // eat effect
                0 => reduce_hunger(&mut agent, 60),
                // get_food effect
                1 => {
                    agent
                        .properties
                        .insert("has_food".to_string(), VariantSnapshot::Bool(true));
                }
                _ => {}
            }
            CallbackResponse::UpdatedSnapshots(agent, world)
        }
        _ => CallbackResponse::Float(1.0),
    });

    let mut engine = PlannerEngine::new(
        inputs(actions, agent, world, req_tx),
        EngineConfig {
            max_depth: 1,
            ..EngineConfig::default()
        },
    );
    let result = run_to_completion(&mut engine, &goals);

    let plan = result.expect("should produce a result");
    assert!(
        !plan.success,
        "Expected failure when max_depth=1 blocks 2-action plan"
    );
}
