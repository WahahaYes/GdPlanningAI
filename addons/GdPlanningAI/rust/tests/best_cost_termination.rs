//! Integration tests for TerminationStrategy::BestCost vs FirstComplete.

mod common;
use common::{inputs, run_to_completion, spawn_responder};

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine, TerminationStrategy};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;

/// Run the planner with the given termination strategy and return the plan result.
fn run_with_strategy(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    strategy: TerminationStrategy,
) -> Option<PlanResult> {
    let mut engine = PlannerEngine::new(
        inputs(actions, agent, world, spawn_responder(answer)),
        EngineConfig {
            max_depth: 10,
            termination: strategy,
            ..EngineConfig::default()
        },
    );
    run_to_completion(&mut engine, &goals)
}

/// Costs: quick 1, slow1 2, slow2 3. quick and slow2 meet the goal; slow1 enables slow2.
fn answer(req: CallbackRequest) -> CallbackResponse {
    match req.kind {
        CallbackKind::GetCost { .. } => {
            let cost = match req.callable_id {
                10 => 1.0,
                20 => 2.0,
                30 => 3.0,
                _ => 1.0,
            };
            CallbackResponse::Float(cost)
        }
        CallbackKind::ApplyEffect { mut agent, world } => {
            match req.callable_id {
                100 => {
                    agent
                        .properties
                        .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
                }
                101 => {
                    agent
                        .properties
                        .insert("can_finish".to_string(), VariantSnapshot::Bool(true));
                }
                102 => {
                    agent
                        .properties
                        .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
                }
                _ => {}
            }
            CallbackResponse::UpdatedSnapshots(agent, world)
        }
        _ => CallbackResponse::Float(1.0),
    }
}

#[test]
fn best_cost_returns_lowest_cost_plan() {
    // Chain A: one action (quick) with cost 1.0, directly satisfies goal.
    // Chain B: two actions (slow1 + slow2) with total cost 5.0, also satisfies goal.
    // BestCost should return Chain A.

    let agent = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    let actions = vec![
        ActionSpec {
            name: "quick".to_string(),
            cost_callable_id: Some(10),
            effect_callable_id: Some(100),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow1".to_string(),
            cost_callable_id: Some(20),
            effect_callable_id: Some(101),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow2".to_string(),
            cost_callable_id: Some(30),
            effect_callable_id: Some(102),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::Equal,
                property_name: "can_finish".to_string(),
                value: Some(VariantSnapshot::Bool(true)),
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "finish".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    let result = run_with_strategy(agent, world, actions, goals, TerminationStrategy::BestCost);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // Chain A (quick) is the cheapest valid plan at cost 1.0
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0); // quick
    assert_eq!(plan.total_cost, 1.0);
}

#[test]
fn first_complete_returns_first_found_plan() {
    // Same setup as best_cost test.
    // With Dijkstra heuristic, the cheapest node is always expanded first,
    // so FirstComplete also finds Chain A (quick, cost 1.0) first.
    // The test documents this behavior.

    let agent = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    let actions = vec![
        ActionSpec {
            name: "quick".to_string(),
            cost_callable_id: Some(10),
            effect_callable_id: Some(100),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow1".to_string(),
            cost_callable_id: Some(20),
            effect_callable_id: Some(101),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow2".to_string(),
            cost_callable_id: Some(30),
            effect_callable_id: Some(102),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::Equal,
                property_name: "can_finish".to_string(),
                value: Some(VariantSnapshot::Bool(true)),
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "finish".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    let result = run_with_strategy(
        agent,
        world,
        actions,
        goals,
        TerminationStrategy::FirstComplete,
    );

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // With Dijkstra heuristic the cheapest complete branch is expanded first,
    // so FirstComplete also returns Chain A (quick, cost 1.0).
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0); // quick
    assert_eq!(plan.total_cost, 1.0);
}
