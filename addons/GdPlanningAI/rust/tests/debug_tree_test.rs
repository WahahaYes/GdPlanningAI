//! Integration tests for the debug tree builder and SearchTree structure.
//!
//! These tests verify that the TreeDump ID-based builder produces a
//! correct SearchTree with proper parent-child relationships, outcomes,
//! excluded actions, and forward-validation steps.

use gdplanningai_rust::debug_tree::{FwdStep, NodeOutcome, SearchTree, TreeDump, TreeNode};

// ── Helpers ────────────────────────────────────────────────────────

fn assert_node(node: &TreeNode, expected_name: Option<&str>, expected_children: usize) {
    assert_eq!(
        node.action_name.as_deref(),
        expected_name,
        "wrong action_name"
    );
    assert_eq!(
        node.children.len(),
        expected_children,
        "wrong child count for {:?}",
        node.action_name
    );
}

fn assert_outcome_pruned(node: &TreeNode, reason_contains: &str) {
    match &node.outcome {
        NodeOutcome::Pruned { reason } => {
            assert!(
                reason.contains(reason_contains),
                "prune reason '{}' does not contain '{}'",
                reason,
                reason_contains
            );
        }
        other => panic!("expected Pruned, got {:?}", other),
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[test]
fn single_goal_already_satisfied() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("eat_goal", 10.0, &["hunger < 30".to_string()]);
    dump.goal_already_satisfied();
    dump.end_goal(true, &[], 0.0);

    let output = dump.format();
    assert!(output.contains("eat_goal"));
    assert!(output.contains("ALREADY SATISFIED"));
}

#[test]
fn goal_with_root_and_no_candidates() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("test_goal", 5.0, &["pre_a".to_string()]);
    let root_id = dump.add_root(&["pre_a".to_string()], &[]);
    dump.set_outcome(root_id, NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    let output = dump.format();
    assert!(output.contains("test_goal"));
    assert!(output.contains("DEAD END"));
    assert!(output.contains("FAILURE"));
}

#[test]
fn single_candidate_completes() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    let root_id = dump.add_root(&["need_x".to_string()], &[]);

    let child_id = dump.add_child(root_id, "do_x", 3.0, 3.0, &[], &[], &[], &[]);
    dump.set_outcome(
        child_id,
        NodeOutcome::Complete {
            chain_len: 1,
            total_cost: 3.0,
            fwd_ok: true,
        },
    );

    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let output = dump.format();
    assert!(output.contains("SUCCESS"));
    assert!(output.contains("do_x"));
    assert!(output.contains("cost=3.00"));
}

#[test]
fn candidate_pruned_by_cost() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    let root_id = dump.add_root(&["need_x".to_string()], &[]);

    let child_id = dump.add_child(
        root_id,
        "expensive_action",
        100.0,
        100.0,
        &[],
        &[],
        &[],
        &[],
    );
    dump.set_outcome(
        child_id,
        NodeOutcome::Pruned {
            reason: "cost 100.00 >= best 5.00".to_string(),
        },
    );

    dump.end_goal(false, &[], 0.0);

    let output = dump.format();
    assert!(output.contains("expensive_action"));
    assert!(output.contains("PRUNED"));
}

#[test]
fn excluded_actions_attached_to_node() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    let root_id = dump.add_root(&["need_x".to_string()], &[]);

    dump.exclude_action(
        root_id,
        "no_effect_action",
        "no effect to satisfy open preconditions",
    );
    dump.exclude_action(
        root_id,
        "freed_action",
        "dependencies invalid (object freed)",
    );

    dump.set_outcome(root_id, NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    let output = dump.format();
    assert!(output.contains("no_effect_action"));
    assert!(output.contains("freed_action"));
}

#[test]
fn forward_validation_steps_attached_to_completing_node() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    let root_id = dump.add_root(&["need_x".to_string()], &[]);

    let child_id = dump.add_child(root_id, "do_x", 3.0, 3.0, &[], &[], &[], &[]);
    dump.add_fwd_step(child_id, "do_x", "dependencies", "valid", true);
    dump.add_fwd_step(child_id, "do_x", "precondition", "1 checks passed", true);
    dump.add_fwd_step(child_id, "do_x", "cost", "3.00", true);
    dump.add_fwd_step(child_id, "GOAL", "goal_check", "all satisfied", true);
    dump.set_outcome(
        child_id,
        NodeOutcome::Complete {
            chain_len: 1,
            total_cost: 3.0,
            fwd_ok: true,
        },
    );

    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let output = dump.format();
    assert!(output.contains("FWD [OK]"));
    assert!(output.contains("dependencies"));
    assert!(output.contains("goal_check"));
}

#[test]
fn multiple_goal_attempts() {
    let mut dump = TreeDump::new_forced();

    // First goal fails
    dump.begin_goal("goal_a", 20.0, &["pre_a".to_string()]);
    let root_a = dump.add_root(&["pre_a".to_string()], &[]);
    dump.set_outcome(root_a, NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    // Second goal succeeds
    dump.begin_goal("goal_b", 10.0, &["pre_b".to_string()]);
    let root_b = dump.add_root(&["pre_b".to_string()], &[]);
    let child_b = dump.add_child(root_b, "do_b", 2.0, 2.0, &[], &[], &[], &[]);
    dump.set_outcome(
        child_b,
        NodeOutcome::Complete {
            chain_len: 1,
            total_cost: 2.0,
            fwd_ok: true,
        },
    );
    dump.end_goal(true, &["do_b".to_string()], 2.0);

    let output = dump.format();
    assert!(output.contains("goal_a"));
    assert!(output.contains("FAILURE"));
    assert!(output.contains("goal_b"));
    assert!(output.contains("SUCCESS"));
}

#[test]
fn branches_counted_by_nodes() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need".to_string()]);
    let root_id = dump.add_root(&["need".to_string()], &[]);

    let a = dump.add_child(root_id, "a", 1.0, 1.0, &[], &[], &[], &[]);
    dump.set_outcome(a, NodeOutcome::DeadEnd);

    let b = dump.add_child(root_id, "b", 2.0, 2.0, &[], &[], &[], &[]);
    dump.set_outcome(
        b,
        NodeOutcome::Pruned {
            reason: "max depth".to_string(),
        },
    );

    let c = dump.add_child(root_id, "c", 3.0, 3.0, &[], &[], &[], &[]);
    dump.set_outcome(
        c,
        NodeOutcome::Complete {
            chain_len: 1,
            total_cost: 3.0,
            fwd_ok: true,
        },
    );

    dump.end_goal(true, &["c".to_string()], 3.0);

    // Root + 3 candidates = 4 branches
    let output = dump.format();
    assert!(output.contains("Branches: 4"));
}

#[test]
fn format_produces_output() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    let root_id = dump.add_root(&["need_x".to_string()], &[]);
    let child_id = dump.add_child(root_id, "do_x", 3.0, 3.0, &[], &[], &[], &[]);
    dump.set_outcome(
        child_id,
        NodeOutcome::Complete {
            chain_len: 1,
            total_cost: 3.0,
            fwd_ok: true,
        },
    );
    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let output = dump.format();
    assert!(output.contains("PLANNER SEARCH TREE"));
    assert!(output.contains("goal"));
    assert!(output.contains("do_x"));
    assert!(output.contains("SUCCESS"));
}

#[test]
fn nested_candidates_build_correct_tree() {
    // Simulate a 2-level search: root → outer_action → inner_action
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["deep_need".to_string()]);
    let root_id = dump.add_root(&["deep_need".to_string()], &[]);

    let outer_id = dump.add_child(
        root_id,
        "outer_action",
        2.0,
        2.0,
        &["inner_need".to_string()],
        &[],
        &[],
        &[],
    );

    let inner_id = dump.add_child(outer_id, "inner_action", 1.0, 3.0, &[], &[], &[], &[]);
    dump.set_outcome(
        inner_id,
        NodeOutcome::Complete {
            chain_len: 2,
            total_cost: 3.0,
            fwd_ok: true,
        },
    );

    dump.end_goal(
        true,
        &["inner_action".to_string(), "outer_action".to_string()],
        3.0,
    );

    let output = dump.format();
    assert!(output.contains("outer_action"));
    assert!(output.contains("inner_action"));
    assert!(output.contains("SUCCESS"));
}
