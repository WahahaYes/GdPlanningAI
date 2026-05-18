//! Integration tests for the backward-chaining planner.
//!
//! These tests exercise `run_plan` end-to-end with a mock callback responder
//! thread, verifying that the planner correctly chains actions to satisfy goals.

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PreconditionSpec,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;

// ── Helpers ────────────────────────────────────────────────────────

fn make_agent(hunger: i64) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(hunger));
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

fn hunger_less_than(threshold: i64) -> PreconditionSpec {
    PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::LessThan,
        property_name: "hunger".to_string(),
        value: Some(VariantSnapshot::Int(threshold)),
    }
}

/// Spawn a thread that responds to callback requests from the planner.
/// Returns the sender to give to the planner and a join handle.
fn spawn_callback_responder(
    cost_value: f64,
    hunger_reduction: i64,
) -> (mpsc::Sender<CallbackRequest>, thread::JoinHandle<()>) {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();

    let handle = thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(cost_value),
                CallbackKind::ApplyEffect { mut agent, world, .. } => {
                    if let Some(VariantSnapshot::Int(current)) =
                        agent.properties.get("hunger").cloned()
                    {
                        let new_hunger = (current - hunger_reduction).max(0);
                        agent
                            .properties
                            .insert("hunger".to_string(), VariantSnapshot::Int(new_hunger));
                    }
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(true),
            };
            let _ = req.response_tx.send(response);
        }
    });

    (req_tx, handle)
}

fn run_planner(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    max_depth: usize,
    request_tx: mpsc::Sender<CallbackRequest>,
) -> Option<PlanResult> {
    let cancel_flag = Arc::new(AtomicBool::new(false));

    gdplanningai_rust::planner::run_plan(
        actions,
        goals,
        agent,
        world,
        vec![], // initial_provisions
        max_depth,
        request_tx,
        cancel_flag,
    )
}

// ── Tests ──────────────────────────────────────────────────────────

#[test]
fn single_action_satisfies_goal() {
    // Agent is hungry (80), goal is hunger < 30.
    // One action "eat" reduces hunger by 60 with cost 5.
    // Planner should find the 1-action plan.
    let agent = make_agent(80);
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    // Drop the sender so the responder thread exits
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0);
    assert_eq!(plan.total_cost, 5.0);
}

#[test]
fn goal_already_satisfied_returns_empty_plan() {
    // Agent hunger is 10, goal is hunger < 30.
    // Goal is already met — planner should return empty plan with zero cost.
    let agent = make_agent(10);
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert!(plan.action_chain.is_empty());
    assert_eq!(plan.total_cost, 0.0);
}

#[test]
fn no_valid_plan_returns_failure() {
    // Agent is hungry (80), goal is hunger < 30.
    // No actions available — planner should return failure.
    let agent = make_agent(80);
    let world = make_world();
    let actions = vec![];
    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn action_with_unmet_precondition_fails() {
    // Agent hunger is 80, goal is hunger < 30.
    // Action "eat" has a precondition "has_food" which is not in the agent state.
    // Planner should find no valid plan.
    let agent = make_agent(80);
    let world = make_world();

    let actions = vec![ActionSpec {
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
    }];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn respects_max_depth() {
    // Agent hunger is 80, goal is hunger < 30.
    // Action "eat" reduces hunger but has a precondition "has_food".
    // Action "get_food" provides "has_food" via its effect.
    // With max_depth=0, no actions can be added to the chain → failure.
    let agent = make_agent(80);
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
            effect_callable_id: Some(0),
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    // max_depth=0: no actions allowed in the chain
    let result = run_planner(agent, world, actions, goals, 0, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn goal_priority_respected() {
    // Two goals: high-priority "hunger < 30" (reward 100) and low-priority
    // "hunger < 80" (reward 1). The high-priority goal requires an action
    // with a precondition that can't be met, but the low-priority goal is
    // already satisfied. The planner should return the low-priority goal
    // as satisfied (empty plan) since the high-priority one fails.
    let agent = make_agent(50); // hunger=50: satisfies <80 but not <30
    let world = make_world();

    let actions = vec![ActionSpec {
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
    }];

    let goals = vec![
        GoalSpec {
            name: "starving".to_string(),
            reward: 100.0,
            desired_state: vec![hunger_less_than(30)],
            original_index: 0,
        },
        GoalSpec {
            name: "peckish".to_string(),
            reward: 1.0,
            desired_state: vec![hunger_less_than(80)],
            original_index: 1,
        },
    ];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // The "peckish" goal (hunger < 80) is already satisfied
    assert_eq!(plan.goal_index, 1);
    assert!(plan.action_chain.is_empty());
}
