//! Focused tests for the `process_simulation` helper refactor.
//!
//! These tests exercise the smaller helpers described in
//! `notes/REFACTOR_PROCESS_SIMULATION.md`. They are written against the
//! semantics described in the note, using the signatures that are exposed in the
//! current source code where possible.
//!
//! Note: the spec describes `collect_bindings_for_position` as a pure function
//! taking `action_bindings`, but the current implementation already exposes it
//! as a public method on [`PlanBranch`] in `planner/types.rs`. The test below
//! uses that method. The other helpers (`clear_initial_state_requirements`,
//! `validate_action_against_current_state`, `evaluate_open_preconditions_for_position`,
//! `clear_requirements_from_provisions`, `finalize_verified_branch`) are
//! assumed to be methods on [`PlannerEngine`] as described in the note.
//!
//! Until the refactor is merged, those `PlannerEngine` helper methods may not
//! exist or may not be public, so `cargo check --tests` is expected to fail with
//! unresolved-name errors. The tests are written against the planned API.

mod common;
use common::{create_sim_object, create_test_agent, create_test_world};

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackRequest, CallbackResponse, PlannerCallback, PreconditionSpec,
};
use gdplanningai_rust::planner::engine::PlannerEngine;
use gdplanningai_rust::planner::simulation::StepResult;
use gdplanningai_rust::planner::types::{BranchState, PlanBranch, SearchContext};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;

// ── Test helpers ───────────────────────────────────────────────────

/// Build a minimal [`PlannerEngine`] with a do-nothing callback thread.
///
/// The resulting engine has a valid [`SearchContext`] so that helpers that read
/// `self.ctx` (initial state, actions, provisions, etc.) can be exercised
/// without a full Godot callback setup.
fn create_test_engine(
    actions: Vec<ActionSpec>,
    initial_agent: BlackboardSnapshot,
    initial_world: BlackboardSnapshot,
    initial_provisions: Vec<ProvisionSpec>,
) -> PlannerEngine {
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // A thread that drains callback requests so that any accidental pending
    // callback does not block the test.
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || while let Ok(_req) = req_rx.recv() {});

    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent,
        initial_world,
        initial_provisions,
        request_tx: req_tx,
        engine_response_tx: engine_tx.clone(),
        discovery_results: std::sync::Mutex::new(HashMap::new()),
        discovery_costs: std::sync::Mutex::new(HashMap::new()),
        discovery_pending: std::sync::Mutex::new(HashMap::new()),
        discovery_request_map: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_results: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_pending: std::sync::Mutex::new(HashMap::new()),
        provision_index: HashMap::new(),
        non_wildcard_actions: vec![],
    });

    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag);
    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;
    engine
}

// ── collect_bindings_for_position ──────────────────────────────────

#[test]
fn collect_bindings_for_position_filters_by_position() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);
    branch.action_bindings = vec![
        (
            0,
            "held_item".to_string(),
            vec![VariantSnapshot::ObjectRef(101)],
        ),
        (
            1,
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(202)],
        ),
        (0, "tool".to_string(), vec![VariantSnapshot::ObjectRef(303)]),
        (
            2,
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(404)],
        ),
    ];

    let pos0 = branch.collect_bindings_for_position(0);
    assert_eq!(pos0.len(), 2);
    assert!(
        pos0.iter()
            .any(|(n, v)| n == "held_item" && v == &[VariantSnapshot::ObjectRef(101)]),
        "expected held_item binding at position 0"
    );
    assert!(
        pos0.iter()
            .any(|(n, v)| n == "tool" && v == &[VariantSnapshot::ObjectRef(303)]),
        "expected tool binding at position 0"
    );

    let pos1 = branch.collect_bindings_for_position(1);
    assert_eq!(pos1.len(), 1);
    assert_eq!(pos1[0].0, "at_target");
    assert_eq!(pos1[0].1, vec![VariantSnapshot::ObjectRef(202)]);

    let pos3 = branch.collect_bindings_for_position(3);
    assert!(pos3.is_empty(), "expected no bindings at position 3");
}

// ── clear_initial_state_requirements ───────────────────────────────

#[test]
fn clear_initial_state_requirements_removes_satisfied_pos_zero() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let initial_provisions = vec![ProvisionSpec::Binding {
        binding_name: "held_item".to_string(),
        value: VariantSnapshot::ObjectRef(101),
    }];

    let actions = vec![ActionSpec {
        name: "noop".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let engine = create_test_engine(actions, agent, world, initial_provisions);
    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.open_requirements = vec![
        (
            0,
            RequirementSpec::BindingExists {
                binding_name: "held_item".to_string(),
            },
        ),
        (
            1,
            RequirementSpec::BindingExists {
                binding_name: "held_item".to_string(),
            },
        ),
        (
            0,
            RequirementSpec::BindingExists {
                binding_name: "missing".to_string(),
            },
        ),
    ];

    engine.clear_initial_state_requirements(&mut branch);

    // The pos-0 requirement satisfied by an initial provision should be gone;
    // the pos-1 and unsatisfied pos-0 requirements should remain.
    assert_eq!(branch.open_requirements.len(), 2);
    assert!(
        branch.open_requirements.iter().any(|(p, r)| {
            *p == 1
                && matches!(
                    r,
                    RequirementSpec::BindingExists { binding_name } if binding_name == "held_item"
                )
        }),
        "expected pos-1 held_item requirement to remain"
    );
    assert!(
        branch.open_requirements.iter().any(|(p, r)| {
            *p == 0
                && matches!(
                    r,
                    RequirementSpec::BindingExists { binding_name } if binding_name == "missing"
                )
        }),
        "expected unsatisfied pos-0 missing requirement to remain"
    );
}

// ── validate_action_against_current_state ──────────────────────────

#[test]
fn validate_action_returns_invalid_for_false_builtin_in_verifying() {
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(50))]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.state = BranchState::Verifying;

    let action = ActionSpec {
        name: "needs_health".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(100)),
        }],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };

    let result = engine.validate_action_against_current_state(&branch, &action, &[], None);
    assert!(
        matches!(result, Some(StepResult::Invalid)),
        "false built-in precondition during Verifying should be Invalid"
    );
}

#[test]
fn validate_action_returns_none_for_satisfied_builtin() {
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(100))]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    let action = ActionSpec {
        name: "needs_health".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)),
        }],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };

    let result = engine.validate_action_against_current_state(&branch, &action, &[], None);
    assert!(
        result.is_none(),
        "satisfied built-in precondition should not short-circuit"
    );
}

#[test]
fn validate_action_skips_open_preconditions_and_custom() {
    // The action has a built-in precondition that is already listed in
    // open_preconditions for the current position, plus a custom precondition.
    // Both should be skipped during re-evaluation.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(50))]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.state = BranchState::Verifying;
    branch.simulation_index = 0;
    branch.open_preconditions = vec![(
        0,
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(100)),
        },
    )];

    let action = ActionSpec {
        name: "needs_health".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![
            PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::GreaterThan,
                property_name: "health".to_string(),
                value: Some(VariantSnapshot::Int(100)),
            },
            PreconditionSpec::Custom {
                callable_id: 1,
                dependent_object_ids: vec![],
            },
        ],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };

    let result = engine.validate_action_against_current_state(&branch, &action, &[], None);
    assert!(
        result.is_none(),
        "open-preconditions and custom preconditions should be skipped"
    );
}

// ── evaluate_open_preconditions_for_position ─────────────────────────

#[test]
fn evaluate_open_preconditions_removes_satisfied_and_returns_ready() {
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(100))]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.simulation_index = 0;
    branch.open_preconditions = vec![
        (
            0,
            PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::GreaterThan,
                property_name: "health".to_string(),
                value: Some(VariantSnapshot::Int(50)),
            },
        ),
        (
            0,
            PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::Equal,
                property_name: "health".to_string(),
                value: Some(VariantSnapshot::Int(100)),
            },
        ),
    ];

    let result = engine.evaluate_open_preconditions_for_position(&mut branch, &[], None);
    assert!(
        matches!(result, Some(StepResult::Ready(()))),
        "satisfied precondition should be removed one-at-a-time and return Ready"
    );
    assert_eq!(
        branch.open_preconditions.len(),
        1,
        "exactly one precondition should have been removed"
    );
}

#[test]
fn evaluate_open_preconditions_returns_invalid_for_false_in_verifying() {
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(10))]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.simulation_index = 0;
    branch.state = BranchState::Verifying;
    branch.open_preconditions = vec![(
        0,
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)),
        },
    )];

    let result = engine.evaluate_open_preconditions_for_position(&mut branch, &[], None);
    assert!(
        matches!(result, Some(StepResult::Invalid)),
        "false open precondition during Verifying should be Invalid"
    );
}

#[test]
fn evaluate_open_preconditions_uses_callback_response_for_custom() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let engine = create_test_engine(vec![], agent, world, vec![]);

    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.simulation_index = 0;
    branch.open_preconditions = vec![(
        0,
        PreconditionSpec::Custom {
            callable_id: 7,
            dependent_object_ids: vec![],
        },
    )];

    let response = CallbackResponse::Bool(true);
    let result = engine.evaluate_open_preconditions_for_position(&mut branch, &[], Some(&response));
    assert!(
        matches!(result, Some(StepResult::Ready(()))),
        "custom precondition with a true callback response should be satisfied and removed"
    );
    assert!(
        branch.open_preconditions.is_empty(),
        "satisfied custom precondition should be removed from the open list"
    );
}

// ── clear_requirements_from_provisions ─────────────────────────────

#[test]
fn clear_requirements_from_provisions_concretizes_wildcard_and_clears_matches() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(
        vec![],
        vec![("obj_01", create_sim_object("obj_01", vec![], vec![]))],
    );

    let actions = vec![ActionSpec {
        name: "go_to".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![ProvisionSpec::FactWildcard {
            fact_name: "at_target".to_string(),
        }],
        dependent_object_ids: vec![],
    }];

    let engine = create_test_engine(actions, agent, world, vec![]);
    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.simulation_index = 0;
    branch.action_chain = vec![0];
    branch.open_requirements = vec![
        (
            1,
            RequirementSpec::Fact {
                fact_name: "at_target".to_string(),
                args: vec![VariantSnapshot::ObjectRef(100)],
            },
        ),
        (
            2,
            RequirementSpec::Fact {
                fact_name: "at_target".to_string(),
                args: vec![VariantSnapshot::ObjectRef(200)],
            },
        ),
    ];

    let bindings = vec![(
        "at_target".to_string(),
        vec![VariantSnapshot::ObjectRef(100)],
    )];
    engine.clear_requirements_from_provisions(&mut branch, 0, &bindings);

    // The FactWildcard provision at_target should be concretized to ObjectRef(100)
    // and clear the matching later requirement, leaving the non-matching one.
    assert_eq!(branch.open_requirements.len(), 1);
    assert!(
        branch.open_requirements.iter().any(|(p, r)| {
            *p == 2
                && matches!(
                    r,
                    RequirementSpec::Fact { fact_name, args }
                    if fact_name == "at_target" && args == &[VariantSnapshot::ObjectRef(200)]
                )
        }),
        "expected non-matching at_target(ObjectRef(200)) requirement to remain"
    );
}

// ── finalize_verified_branch ────────────────────────────────────────

#[test]
fn finalize_verified_branch_records_best_plan_when_complete() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let actions = vec![ActionSpec {
        name: "noop".to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let mut engine = create_test_engine(actions, agent, world, vec![]);
    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.state = BranchState::Verifying;
    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    branch.simulation_index = branch.action_chain.len(); // end of chain
    branch.recalculate_cost();

    let result = engine.finalize_verified_branch(&mut branch);
    assert!(
        matches!(result, StepResult::Complete),
        "complete verified branch should return Complete"
    );
    assert!(
        engine.best_plan.is_some(),
        "best_plan should be recorded for a complete verified branch"
    );
    assert_eq!(engine.best_cost, 1.0);
    let plan = engine.best_plan.unwrap();
    assert!(plan.success);
    assert_eq!(plan.action_chain, vec![0i64]);
    assert_eq!(plan.total_cost, 1.0);
    assert_eq!(plan.goal_index, 0);
}

#[test]
fn finalize_verified_branch_invalid_when_open_needs_remain() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut engine = create_test_engine(vec![], agent, world, vec![]);
    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.state = BranchState::Verifying;
    branch.action_chain = vec![];
    branch.simulation_index = 0; // end of empty chain
    branch.open_preconditions = vec![(
        0,
        PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "health".to_string(),
            value: None,
        },
    )];

    let result = engine.finalize_verified_branch(&mut branch);
    assert!(
        matches!(result, StepResult::Invalid),
        "open preconditions at end-of-chain should be Invalid"
    );
    assert!(
        engine.best_plan.is_none(),
        "best_plan should not be recorded when open needs remain"
    );
}

#[test]
fn finalize_verified_branch_searching_returns_ready() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut engine = create_test_engine(vec![], agent, world, vec![]);
    let mut branch = PlanBranch::new(&engine.ctx.initial_agent, &engine.ctx.initial_world);
    branch.state = BranchState::Searching;
    branch.action_chain = vec![];
    branch.simulation_index = 0;

    let result = engine.finalize_verified_branch(&mut branch);
    assert!(
        matches!(result, StepResult::Ready(())),
        "Searching state at end-of-chain should return Ready to continue search"
    );
}
