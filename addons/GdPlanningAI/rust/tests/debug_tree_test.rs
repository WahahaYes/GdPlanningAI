//! Integration tests for the debug tree builder and SearchTree structure.
//!
//! These tests verify that the TreeDump stack-based builder produces a
//! correct SearchTree with proper parent-child relationships, outcomes,
//! excluded actions, and forward-validation steps.

use gdplanningai_rust::debug_tree::{FwdStep, NodeOutcome, SearchNode, SearchTree, TreeDump};

// ── Helpers ────────────────────────────────────────────────────────

fn assert_node(node: &SearchNode, expected_name: Option<&str>, expected_children: usize) {
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

fn assert_outcome_pruned(node: &SearchNode, reason_contains: &str) {
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

    let tree = dump.finish();
    assert_eq!(tree.goal_attempts.len(), 1);
    let ga = &tree.goal_attempts[0];
    assert_eq!(ga.goal_name, "eat_goal");
    assert!(ga.already_satisfied);
    assert!(ga.success);
    assert!(ga.root.is_none());
    assert!(ga.plan_actions.is_empty());
}

#[test]
fn goal_with_root_and_no_candidates() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("test_goal", 5.0, &["pre_a".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["pre_a".to_string()], &[]);
    dump.exit_node(NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    let tree = dump.finish();
    let ga = &tree.goal_attempts[0];
    assert!(!ga.success);
    let root = ga.root.as_ref().unwrap();
    assert_node(root, None, 0);
    assert!(matches!(root.outcome, NodeOutcome::DeadEnd));
}

#[test]
fn single_candidate_completes() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);

    // Candidate action that satisfies the need
    dump.enter_node(Some("do_x"), 3.0, 3.0, &[], &[]);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 1,
        total_cost: 3.0,
        fwd_ok: true,
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let tree = dump.finish();
    let ga = &tree.goal_attempts[0];
    assert!(ga.success);
    assert_eq!(ga.plan_actions, vec!["do_x"]);
    assert_eq!(ga.plan_cost, 3.0);

    let root = ga.root.as_ref().unwrap();
    assert_node(root, None, 1);
    assert!(matches!(root.outcome, NodeOutcome::Expanded));

    let child = &root.children[0];
    assert_node(child, Some("do_x"), 0);
    assert!(matches!(child.outcome, NodeOutcome::Complete { .. }));
}

#[test]
fn candidate_pruned_by_cost() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);

    dump.enter_node(Some("expensive_action"), 100.0, 100.0, &[], &[]);
    dump.exit_node(NodeOutcome::Pruned {
        reason: "cost 100.00 >= best 5.00".to_string(),
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(false, &[], 0.0);

    let tree = dump.finish();
    let root = tree.goal_attempts[0].root.as_ref().unwrap();
    let child = &root.children[0];
    assert_outcome_pruned(child, "cost");
}

#[test]
fn candidate_skipped() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);

    dump.enter_node(Some("broken_action"), 1.0, 1.0, &[], &[]);
    dump.exit_node(NodeOutcome::Skipped {
        reason: "could not resolve pending effects".to_string(),
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(false, &[], 0.0);

    let tree = dump.finish();
    let child = &tree.goal_attempts[0].root.as_ref().unwrap().children[0];
    assert!(matches!(child.outcome, NodeOutcome::Skipped { .. }));
}

#[test]
fn excluded_actions_attached_to_node() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);

    dump.exclude_action(
        "no_effect_action",
        "no effect to satisfy open preconditions",
    );
    dump.exclude_action("freed_action", "dependencies invalid (object freed)");

    dump.exit_node(NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    let tree = dump.finish();
    let root = tree.goal_attempts[0].root.as_ref().unwrap();
    assert_eq!(root.excluded_actions.len(), 2);
    assert_eq!(root.excluded_actions[0].action_name, "no_effect_action");
    assert_eq!(root.excluded_actions[1].action_name, "freed_action");
}

#[test]
fn forward_validation_steps_attached_to_completing_node() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);

    dump.enter_node(Some("do_x"), 3.0, 3.0, &[], &[]);
    dump.add_fwd_step("do_x", "dependencies", "valid", true);
    dump.add_fwd_step("do_x", "precondition", "1 checks passed", true);
    dump.add_fwd_step("do_x", "cost", "3.00", true);
    dump.add_fwd_step("GOAL", "goal_check", "all satisfied", true);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 1,
        total_cost: 3.0,
        fwd_ok: true,
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let tree = dump.finish();
    let child = &tree.goal_attempts[0].root.as_ref().unwrap().children[0];
    assert_eq!(child.forward_validation.len(), 4);
    assert_eq!(child.forward_validation[0].step, "dependencies");
    assert_eq!(child.forward_validation[3].action_name, "GOAL");
}

#[test]
fn multiple_goal_attempts() {
    let mut dump = TreeDump::new_forced();

    // First goal fails
    dump.begin_goal("goal_a", 20.0, &["pre_a".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["pre_a".to_string()], &[]);
    dump.exit_node(NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    // Second goal succeeds
    dump.begin_goal("goal_b", 10.0, &["pre_b".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["pre_b".to_string()], &[]);
    dump.enter_node(Some("do_b"), 2.0, 2.0, &[], &[]);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 1,
        total_cost: 2.0,
        fwd_ok: true,
    });
    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(true, &["do_b".to_string()], 2.0);

    let tree = dump.finish();
    assert_eq!(tree.goal_attempts.len(), 2);
    assert!(!tree.goal_attempts[0].success);
    assert!(tree.goal_attempts[1].success);
}

#[test]
fn branches_explored_counted() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need".to_string()], &[]);

    // Three candidates
    dump.enter_node(Some("a"), 1.0, 1.0, &[], &[]);
    dump.exit_node(NodeOutcome::DeadEnd);

    dump.enter_node(Some("b"), 2.0, 2.0, &[], &[]);
    dump.exit_node(NodeOutcome::Pruned {
        reason: "max depth".to_string(),
    });

    dump.enter_node(Some("c"), 3.0, 3.0, &[], &[]);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 1,
        total_cost: 3.0,
        fwd_ok: true,
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(true, &["c".to_string()], 3.0);

    let tree = dump.finish();
    // Root + 3 candidates = 4 branches
    assert_eq!(tree.branches_explored, 4);
}

#[test]
fn elapsed_time_is_positive() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need".to_string()], &[]);
    dump.exit_node(NodeOutcome::DeadEnd);
    dump.end_goal(false, &[], 0.0);

    let tree = dump.finish();
    assert!(tree.elapsed_ms >= 0.0);
}

#[test]
fn format_produces_output() {
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["need_x".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["need_x".to_string()], &[]);
    dump.enter_node(Some("do_x"), 3.0, 3.0, &[], &[]);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 1,
        total_cost: 3.0,
        fwd_ok: true,
    });
    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(true, &["do_x".to_string()], 3.0);

    let output = dump.format();
    assert!(output.contains("PLANNER SEARCH TREE"));
    assert!(output.contains("goal"));
    assert!(output.contains("do_x"));
    assert!(output.contains("SUCCESS"));
}

#[test]
fn nested_candidates_build_correct_tree() {
    // Simulate a 2-level search: root → candidate_a → candidate_a1
    let mut dump = TreeDump::new_forced();
    dump.begin_goal("goal", 10.0, &["deep_need".to_string()]);
    dump.enter_node(None, 0.0, 0.0, &["deep_need".to_string()], &[]);

    // First-level candidate
    dump.enter_node(
        Some("outer_action"),
        2.0,
        2.0,
        &["inner_need".to_string()],
        &[],
    );

    // Second-level candidate (child of outer_action)
    dump.enter_node(Some("inner_action"), 1.0, 3.0, &[], &[]);
    dump.exit_node(NodeOutcome::Complete {
        chain_len: 2,
        total_cost: 3.0,
        fwd_ok: true,
    });

    dump.exit_node(NodeOutcome::Expanded);
    dump.exit_node(NodeOutcome::Expanded);
    dump.end_goal(
        true,
        &["inner_action".to_string(), "outer_action".to_string()],
        3.0,
    );

    let tree = dump.finish();
    let root = tree.goal_attempts[0].root.as_ref().unwrap();
    assert_node(root, None, 1);

    let outer = &root.children[0];
    assert_node(outer, Some("outer_action"), 1);

    let inner = &outer.children[0];
    assert_node(inner, Some("inner_action"), 0);
    assert!(matches!(inner.outcome, NodeOutcome::Complete { .. }));
}
