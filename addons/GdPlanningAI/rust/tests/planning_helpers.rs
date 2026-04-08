//! Tests for planning algorithm helper functions.
//!
//! Tests logic that can be evaluated without requiring callback channels,
//! focusing on builtin precondition evaluation and goal satisfaction logic.

mod common;

use common::create_test_agent;
use gdplanningai_rust::background_types::PreconditionSpec;
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::VariantSnapshot;

// Note: We can't directly test background_plan.rs functions since they're private,
// but we can test the underlying logic through PreconditionSpec::evaluate_builtin
// which is what those functions use for builtin preconditions.

#[test]
fn goal_satisfied_when_all_preconditions_true() {
    // Simulates is_goal_satisfied() logic for builtin preconditions.
    // All preconditions must evaluate to true for the goal to be satisfied.
    let agent = create_test_agent(vec![
        ("health", VariantSnapshot::Int(100)),
        ("ammo", VariantSnapshot::Int(20)),
    ]);
    let world = create_test_agent(vec![("time_of_day", VariantSnapshot::Str("day".to_string()))]);

    let goal_preconditions = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)),
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "ammo".to_string(),
            value: Some(VariantSnapshot::Int(10)),
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::WorldState,
            operation: PreconditionOp::Equal,
            property_name: "time_of_day".to_string(),
            value: Some(VariantSnapshot::Str("day".to_string())),
        },
    ];

    // All preconditions should be satisfied
    let all_satisfied = goal_preconditions
        .iter()
        .all(|p| p.evaluate_builtin(&agent, &world) == Some(true));

    assert!(all_satisfied);
}

#[test]
fn goal_not_satisfied_when_any_precondition_false() {
    // Goal satisfaction requires ALL preconditions to be true.
    // If any single precondition fails, the goal is not satisfied.
    let agent = create_test_agent(vec![
        ("health", VariantSnapshot::Int(30)), // Too low!
        ("ammo", VariantSnapshot::Int(20)),
    ]);
    let world = create_test_agent(vec![]);

    let goal_preconditions = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)), // This will fail
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "ammo".to_string(),
            value: Some(VariantSnapshot::Int(10)), // This would pass
        },
    ];

    let all_satisfied = goal_preconditions
        .iter()
        .all(|p| p.evaluate_builtin(&agent, &world) == Some(true));

    assert!(!all_satisfied);
}

#[test]
fn progress_toward_goal_when_any_precondition_satisfied() {
    // Simulates check_progress_toward_goal() logic.
    // Returns true if at least ONE precondition is satisfied.
    let agent = create_test_agent(vec![
        ("has_key", VariantSnapshot::Bool(false)),
        ("health", VariantSnapshot::Int(80)),
    ]);
    let world = create_test_agent(vec![]);

    let goal_preconditions = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "has_key".to_string(),
            value: Some(VariantSnapshot::Bool(true)), // Not satisfied
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)), // Satisfied!
        },
    ];

    let makes_progress = goal_preconditions
        .iter()
        .any(|p| p.evaluate_builtin(&agent, &world) == Some(true));

    assert!(makes_progress);
}

#[test]
fn no_progress_when_no_preconditions_satisfied() {
    // No progress is made if none of the goal's preconditions are met.
    // This means the action doesn't move us closer to the goal.
    let agent = create_test_agent(vec![
        ("has_weapon", VariantSnapshot::Bool(false)),
        ("level", VariantSnapshot::Int(1)),
    ]);
    let world = create_test_agent(vec![]);

    let goal_preconditions = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "has_weapon".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "level".to_string(),
            value: Some(VariantSnapshot::Int(5)),
        },
    ];

    let makes_progress = goal_preconditions
        .iter()
        .any(|p| p.evaluate_builtin(&agent, &world) == Some(true));

    assert!(!makes_progress);
}

#[test]
fn empty_goal_preconditions_is_always_satisfied() {
    // A goal with no preconditions is trivially satisfied.
    // This edge case should result in an empty plan with zero cost.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let goal_preconditions: Vec<PreconditionSpec> = vec![];

    let all_satisfied = goal_preconditions
        .iter()
        .all(|p| p.evaluate_builtin(&agent, &world) == Some(true));

    assert!(all_satisfied);
}

#[test]
fn action_validity_all_checks_pass() {
    // Simulates action_is_valid() logic for builtin validity checks.
    // All validity checks must pass for the action to be considered valid.
    let agent = create_test_agent(vec![
        ("stamina", VariantSnapshot::Int(50)),
        ("can_move", VariantSnapshot::Bool(true)),
    ]);
    let world = create_test_agent(vec![]);

    let validity_checks = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "stamina".to_string(),
            value: Some(VariantSnapshot::Int(10)),
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "can_move".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        },
    ];

    let is_valid = validity_checks
        .iter()
        .all(|check| check.evaluate_builtin(&agent, &world) == Some(true));

    assert!(is_valid);
}

#[test]
fn action_invalid_when_validity_check_fails() {
    // If any validity check fails, the action is not valid for the current state.
    // The planning algorithm will skip this action.
    let agent = create_test_agent(vec![
        ("stamina", VariantSnapshot::Int(5)), // Too low!
        ("can_move", VariantSnapshot::Bool(true)),
    ]);
    let world = create_test_agent(vec![]);

    let validity_checks = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "stamina".to_string(),
            value: Some(VariantSnapshot::Int(10)), // This fails
        },
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "can_move".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        },
    ];

    let is_valid = validity_checks
        .iter()
        .all(|check| check.evaluate_builtin(&agent, &world) == Some(true));

    assert!(!is_valid);
}

#[test]
fn multiple_goals_sorted_by_reward() {
    // Simulates the goal sorting logic in run_plan().
    // Goals are processed in descending order by reward (highest first).
    let mut goals = vec![
        ("low_reward", 10.0),
        ("high_reward", 100.0),
        ("medium_reward", 50.0),
    ];

    goals.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));

    assert_eq!(goals[0].0, "high_reward");
    assert_eq!(goals[1].0, "medium_reward");
    assert_eq!(goals[2].0, "low_reward");
}

#[test]
fn precondition_propagation_extends_desired_state() {
    // When an action's preconditions aren't met, they're added to the desired state
    // for the next recursion level. This tests the Vec extension pattern.
    let mut desired_state = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "goal_condition".to_string(),
            value: None,
        },
    ];

    let action_preconditions = vec![
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "action_requirement".to_string(),
            value: None,
        },
    ];

    let original_len = desired_state.len();
    desired_state.extend(action_preconditions.clone());

    assert_eq!(desired_state.len(), original_len + action_preconditions.len());
    assert_eq!(desired_state.len(), 2);
}

// Integration test notes:
// - run_plan() requires callback channels for custom preconditions and action effects
// - build_plan_recursive() needs CallbackRequest/CallbackResponse mocking
// - check_dependencies_valid() requires Godot runtime to validate InstanceIds
// - call_get_cost(), call_apply_effect(), call_eval_custom_precond() all need channel setup
//
// These should be tested in Phase 4 with proper Godot integration tests or in
// manual testing within the Godot project.
