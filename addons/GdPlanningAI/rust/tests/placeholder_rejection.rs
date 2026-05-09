//! Integration tests verifying that placeholder-simulated actions are rejected.
//!
//! These tests ensure that when an action's requirements are satisfied by predecessor actions,
//! the action is re-simulated with concrete values rather than keeping placeholder values.

use gdplanningai_rust::plan_tree::{PlanResult, PlanTreeNode, extract_best_plan};

/// A plan with no deferred actions should be accepted (empty deferred_indices).
#[test]
fn plan_with_no_deferred_actions_is_valid() {
    // Simulate a simple valid plan tree where all actions were concretely simulated
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![PlanTreeNode {
            action_index: 0,
            cost: 5.0,
            children: vec![PlanTreeNode {
                action_index: 1,
                cost: 3.0,
                children: vec![],
                was_concretely_simulated: true,
            }],
            was_concretely_simulated: true,
        }],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);

    // Verify no deferred actions in the extracted plan
    assert!(
        plan.deferred_indices.is_empty(),
        "Valid plan should have no deferred actions, but got: {:?}",
        plan.deferred_indices
    );
    assert_eq!(plan.actions, vec![0, 1]);
    assert_eq!(plan.cost, 8.0);
}

/// When only placeholder plans exist, extract_best_plan selects the cheapest placeholder.
/// (This would be rejected by the planner later, but extraction tracks it.)
#[test]
fn placeholder_plan_selected_when_no_concrete_alternative() {
    // Tree with only placeholder-simulated actions
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![PlanTreeNode {
            action_index: 0,
            cost: 5.0,
            children: vec![PlanTreeNode {
                action_index: 1,
                cost: 1.0, // Placeholder cost
                children: vec![],
                was_concretely_simulated: false, // Deferred!
            }],
            was_concretely_simulated: true,
        }],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);

    // Only placeholder exists, so it gets selected (would be rejected by planner)
    assert_eq!(plan.deferred_indices, vec![1]);
    assert_eq!(plan.actions, vec![0, 1]);
}

/// Verify that extract_best_plan prefers concrete plans over cheaper placeholder plans.
#[test]
fn concrete_plan_preferred_over_cheaper_placeholder() {
    // Tree with multiple branches - concrete is more expensive than placeholder
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![
            // Branch 1: All concrete (cost 5 + 3 = 8)
            PlanTreeNode {
                action_index: 0,
                cost: 5.0,
                children: vec![PlanTreeNode {
                    action_index: 1,
                    cost: 3.0,
                    children: vec![],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            },
            // Branch 2: Has deferred action (cost 2 + 1 placeholder = 3)
            // This branch is cheaper but contains deferred action - should be IGNORED
            PlanTreeNode {
                action_index: 2,
                cost: 2.0,
                children: vec![PlanTreeNode {
                    action_index: 3,
                    cost: 1.0, // Placeholder
                    children: vec![],
                    was_concretely_simulated: false, // Deferred
                }],
                was_concretely_simulated: true,
            },
        ],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);

    // Should pick the concrete path (branch 1 with cost 8), NOT the cheaper placeholder path
    assert_eq!(plan.cost, 8.0);
    assert_eq!(plan.actions, vec![0, 1]);
    // No deferred actions in the selected plan
    assert!(plan.deferred_indices.is_empty());
}

/// Verify PlanResult::failure() has empty deferred_action_indices.
#[test]
fn failure_result_has_empty_deferred_indices() {
    let failure = PlanResult::failure();
    assert!(failure.deferred_action_indices.is_empty());
}
