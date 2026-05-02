//! Integration tests for plan tree construction and best path extraction.
//!
//! Tests the planning algorithm's tree traversal and cost optimization logic.

use gdplanningai_rust::plan_tree::{PlanResult, PlanTreeNode, extract_best_plan};

#[test]
fn extract_best_plan_single_action() {
    // A simple plan tree with one action should extract that action with its cost.
    // Verifies the basic extraction logic works for trivial cases.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![PlanTreeNode {
            action_index: 0,
            cost: 5.0,
            children: vec![],
            was_concretely_simulated: true,
        }],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions, vec![0]);
    assert_eq!(plan.cost, 5.0);
}

#[test]
fn extract_best_plan_picks_lowest_cost_branch() {
    // When multiple single-action branches exist, choose the one with lowest cost.
    // Tests the optimization logic that selects the cheapest plan.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![
            PlanTreeNode {
                action_index: 0,
                cost: 10.0,
                children: vec![],
                was_concretely_simulated: true,
            },
            PlanTreeNode {
                action_index: 1,
                cost: 5.0,
                children: vec![],
                was_concretely_simulated: true,
            },
            PlanTreeNode {
                action_index: 2,
                cost: 7.0,
                children: vec![],
                was_concretely_simulated: true,
            },
        ],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions, vec![1]);
    assert_eq!(plan.cost, 5.0);
}

#[test]
fn extract_best_plan_multi_step_chain() {
    // A linear chain of actions should accumulate costs correctly.
    // Verifies that multi-step plans sum their action costs properly.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![PlanTreeNode {
            action_index: 0,
            cost: 3.0,
            children: vec![PlanTreeNode {
                action_index: 1,
                cost: 4.0,
                children: vec![PlanTreeNode {
                    action_index: 2,
                    cost: 2.0,
                    children: vec![],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            }],
            was_concretely_simulated: true,
        }],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions, vec![0, 1, 2]);
    assert_eq!(plan.cost, 9.0); // 3.0 + 4.0 + 2.0
}

#[test]
fn extract_best_plan_picks_cheapest_multi_step_path() {
    // With multiple multi-step paths, select the one with lowest total cost.
    // Tests optimization across different path lengths.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![
            // Expensive two-step path: 10 + 5 = 15
            PlanTreeNode {
                action_index: 0,
                cost: 10.0,
                children: vec![PlanTreeNode {
                    action_index: 1,
                    cost: 5.0,
                    children: vec![],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            },
            // Cheap two-step path: 2 + 3 = 5
            PlanTreeNode {
                action_index: 2,
                cost: 2.0,
                children: vec![PlanTreeNode {
                    action_index: 3,
                    cost: 3.0,
                    children: vec![],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            },
        ],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions, vec![2, 3]);
    assert_eq!(plan.cost, 5.0);
}

#[test]
fn extract_best_plan_empty_root_returns_zero_cost() {
    // An empty plan tree (no children) should return an empty plan with zero cost.
    // Tests the edge case where no valid plan exists.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions.len(), 0);
    assert_eq!(plan.cost, 0.0);
}

#[test]
fn extract_best_plan_complex_branching() {
    // Complex tree with multiple branches of varying lengths and costs.
    // Ensures the algorithm correctly explores all paths and picks the optimal one.
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![
            // Branch 1: Single action (cost 8)
            PlanTreeNode {
                action_index: 0,
                cost: 8.0,
                children: vec![],
                was_concretely_simulated: true,
            },
            // Branch 2: Two-step (cost 3 + 2 = 5) <- should win
            PlanTreeNode {
                action_index: 1,
                cost: 3.0,
                children: vec![PlanTreeNode {
                    action_index: 2,
                    cost: 2.0,
                    children: vec![],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            },
            // Branch 3: Three-step (cost 2 + 2 + 3 = 7)
            PlanTreeNode {
                action_index: 3,
                cost: 2.0,
                children: vec![PlanTreeNode {
                    action_index: 4,
                    cost: 2.0,
                    children: vec![PlanTreeNode {
                        action_index: 5,
                        cost: 3.0,
                        children: vec![],
                        was_concretely_simulated: true,
                    }],
                    was_concretely_simulated: true,
                }],
                was_concretely_simulated: true,
            },
        ],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.actions, vec![1, 2]);
    assert_eq!(plan.cost, 5.0);
}

#[test]
fn plan_result_failure_defaults() {
    // PlanResult::failure() should initialize with sensible failure defaults.
    // Verifies the failure constructor sets success=false and infinite cost.
    let failure = PlanResult::failure();

    assert_eq!(failure.success, false);
    assert_eq!(failure.action_chain.len(), 0);
    assert_eq!(failure.total_cost, f64::INFINITY);
    assert_eq!(failure.goal_index, -1);
}

#[test]
fn plan_result_can_be_cloned() {
    // PlanResult should implement Clone and create independent copies.
    // Tests that all fields are correctly copied.
    let original = PlanResult {
        success: true,
        action_chain: vec![0, 1, 2],
        total_cost: 15.0,
        goal_index: 3,
        deferred_action_indices: vec![],
    };

    let cloned = original.clone();

    assert_eq!(cloned.success, original.success);
    assert_eq!(cloned.action_chain, original.action_chain);
    assert_eq!(cloned.total_cost, original.total_cost);
    assert_eq!(cloned.goal_index, original.goal_index);
}

#[test]
fn plan_tree_node_clone_creates_deep_copy() {
    // PlanTreeNode clone should deep-copy the entire tree structure.
    // Ensures child nodes are cloned, not just referenced.
    let original = PlanTreeNode {
        action_index: 0,
        cost: 5.0,
        children: vec![PlanTreeNode {
            action_index: 1,
            cost: 3.0,
            children: vec![],
            was_concretely_simulated: true,
        }],
        was_concretely_simulated: true,
    };

    let cloned = original.clone();

    assert_eq!(cloned.action_index, original.action_index);
    assert_eq!(cloned.cost, original.cost);
    assert_eq!(cloned.children.len(), original.children.len());
    assert_eq!(cloned.children[0].action_index, 1);
}

#[test]
fn extract_best_plan_handles_ties_deterministically() {
    // When multiple plans have identical costs, pick the first one encountered.
    // Tests deterministic behavior for tie-breaking (important for reproducibility).
    let root = PlanTreeNode {
        action_index: -1,
        cost: 0.0,
        children: vec![
            PlanTreeNode {
                action_index: 0,
                cost: 5.0,
                children: vec![],
                was_concretely_simulated: true,
            },
            PlanTreeNode {
                action_index: 1,
                cost: 5.0,
                children: vec![],
                was_concretely_simulated: true,
            },
        ],
        was_concretely_simulated: true,
    };

    let plan = extract_best_plan(&root);
    assert_eq!(plan.cost, 5.0);
    // Should pick the first branch (action 0) since costs are equal
    assert_eq!(plan.actions, vec![0]);
}
