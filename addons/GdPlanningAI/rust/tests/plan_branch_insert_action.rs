//! Focused tests for [`PlanBranch::insert_action_at`].
//!
//! These tests exercise the insertion contract directly without spinning up a
//! full [`PlannerEngine`].

mod common;
use common::{create_test_agent, create_test_world};

use gdplanningai_rust::plan_types::{ActionSpec, PreconditionSpec};
use gdplanningai_rust::planner::types::PlanBranch;
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::VariantSnapshot;
use std::collections::HashSet;

fn make_req(name: &str, arg: i64) -> RequirementSpec {
    RequirementSpec::Fact {
        fact_name: name.to_string(),
        args: vec![VariantSnapshot::ObjectRef(arg)],
    }
}

fn make_prov(name: &str, arg: i64) -> ProvisionSpec {
    ProvisionSpec::Fact {
        fact_name: name.to_string(),
        args: vec![VariantSnapshot::ObjectRef(arg)],
    }
}

fn empty_action(name: &str) -> ActionSpec {
    ActionSpec {
        name: name.to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }
}

#[test]
fn insert_first_action_into_empty_branch() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    let bindings = branch.insert_action_at(0, 7, 1.5, vec![], HashSet::new(), vec![], vec![]);

    assert_eq!(branch.action_chain, vec![7]);
    assert_eq!(branch.action_costs, vec![1.5]);
    assert!(branch.open_preconditions.is_empty());
    assert!(branch.open_requirements.is_empty());
    assert!(branch.action_bindings.is_empty());
    assert!(bindings.is_empty());
    assert_eq!(branch.cost, 0.0);
}

#[test]
fn insert_before_consumer_shifts_positions() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    branch.open_requirements = vec![(1, make_req("at_target", 100))];

    let new_req = make_req("held_item", 200);
    branch.insert_action_at(
        1,
        1,
        2.0,
        vec![(0, make_req("at_target", 100), make_prov("at_target", 100))],
        HashSet::new(),
        vec![],
        vec![new_req.clone()],
    );

    assert_eq!(branch.action_chain, vec![0, 1]);
    assert_eq!(branch.action_costs, vec![1.0, 2.0]);

    // The consumed requirement was at position 1 before the shift; after the
    // shift it would have been at 2, but it is removed. The newly added
    // requirement sits at the new action's position (1).
    assert_eq!(branch.open_requirements.len(), 1);
    assert_eq!(branch.open_requirements[0].0, 1);
    assert_eq!(branch.open_requirements[0].1, new_req);
}

#[test]
fn greedy_clears_identical_requirements() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    // Two identical requirements at positions 1 and 2.
    branch.open_requirements = vec![
        (1, make_req("at_target", 100)),
        (2, make_req("at_target", 100)),
    ];

    branch.insert_action_at(
        1,
        1,
        2.0,
        vec![(0, make_req("at_target", 100), make_prov("at_target", 100))],
        HashSet::new(),
        vec![],
        vec![],
    );

    assert!(branch.open_requirements.is_empty());
}

#[test]
fn records_provider_and_consumer_bindings() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    branch.open_requirements = vec![
        (1, make_req("at_target", 100)),
        (2, make_req("at_target", 100)),
    ];

    let bindings = branch.insert_action_at(
        1,
        1,
        2.0,
        vec![(0, make_req("at_target", 100), make_prov("at_target", 100))],
        HashSet::new(),
        vec![],
        vec![],
    );

    // After shift, the two consumers are at positions 2 and 3.
    let expected = vec![
        (
            1,
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(100)],
        ),
        (
            2,
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(100)],
        ),
        (
            3,
            "at_target".to_string(),
            vec![VariantSnapshot::ObjectRef(100)],
        ),
    ];
    assert_eq!(bindings, expected);
    assert_eq!(branch.action_bindings, expected);
}

#[test]
fn insert_at_end_does_not_shift_existing_entries() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    branch.action_chain = vec![0, 1];
    branch.action_costs = vec![1.0, 2.0];
    branch.open_requirements = vec![(2, make_req("held_item", 200))];

    let new_req = make_req("extra", 300);
    branch.insert_action_at(
        2,
        2,
        3.0,
        vec![],
        HashSet::new(),
        vec![],
        vec![new_req.clone()],
    );

    assert_eq!(branch.action_chain, vec![0, 1, 2]);
    assert_eq!(branch.action_costs, vec![1.0, 2.0, 3.0]);
    // The existing open requirement is at the end position (2), which is where
    // the new action is inserted, so it shifts to 3.
    assert_eq!(branch.open_requirements.len(), 2);
    assert!(
        branch
            .open_requirements
            .contains(&(3, make_req("held_item", 200)))
    );
    assert!(branch.open_requirements.contains(&(2, new_req)));
}

#[test]
fn removes_satisfied_preconditions_and_adds_new_ones() {
    let agent = create_test_agent(vec![]);
    let world = create_test_world(vec![], vec![]);
    let mut branch = PlanBranch::new(&agent, &world);

    branch.action_chain = vec![0];
    branch.action_costs = vec![1.0];
    let old_pre = PreconditionSpec::Custom {
        callable_id: 1,
        dependent_object_ids: vec![],
    };
    let new_pre = PreconditionSpec::Custom {
        callable_id: 2,
        dependent_object_ids: vec![],
    };
    branch.open_preconditions = vec![(1, old_pre.clone())];

    let mut pre_indices = HashSet::new();
    pre_indices.insert(0);

    branch.insert_action_at(
        1,
        1,
        2.0,
        vec![],
        pre_indices,
        vec![new_pre.clone()],
        vec![],
    );

    assert!(
        !branch.open_preconditions.iter().any(|(_, p)| p == &old_pre),
        "satisfied precondition should be removed"
    );
    assert_eq!(branch.open_preconditions.len(), 1);
    assert_eq!(branch.open_preconditions[0].0, 1);
    assert_eq!(branch.open_preconditions[0].1, new_pre);
}
