//! Integration tests for snapshot-based precondition evaluation.
//!
//! These tests verify that PreconditionSpec can evaluate builtin operations
//! against BlackboardSnapshot without requiring the Godot runtime.

mod common;

use common::{create_sim_object, create_test_agent, create_test_world};
use gdplanningai_rust::plan_types::PreconditionSpec;
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::VariantSnapshot;

#[test]
fn builtin_has_property_on_agent() {
    // HasProperty should return true when the property exists on the agent blackboard.
    // This verifies the basic property existence check works on agent state.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(100))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::HasProperty,
        property_name: "health".to_string(),
        value: None,
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn builtin_has_property_missing_on_agent() {
    // HasProperty should return false when the property does not exist.
    // Ensures missing properties are correctly detected.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::HasProperty,
        property_name: "missing_key".to_string(),
        value: None,
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn builtin_equal_int_matches() {
    // Equal operation should return true when integer values match exactly.
    // Tests the basic equality comparison for int snapshots.
    let agent = create_test_agent(vec![("ammo", VariantSnapshot::Int(10))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::Equal,
        property_name: "ammo".to_string(),
        value: Some(VariantSnapshot::Int(10)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn builtin_equal_int_mismatch() {
    // Equal operation should return false when integer values don't match.
    // Verifies inequality is correctly detected.
    let agent = create_test_agent(vec![("ammo", VariantSnapshot::Int(5))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::Equal,
        property_name: "ammo".to_string(),
        value: Some(VariantSnapshot::Int(10)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn builtin_greater_than_succeeds() {
    // GreaterThan should return true when the actual value exceeds the threshold.
    // Tests numeric comparison logic for planning preconditions.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(75))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::GreaterThan,
        property_name: "health".to_string(),
        value: Some(VariantSnapshot::Int(50)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn builtin_greater_than_fails() {
    // GreaterThan should return false when the actual value is below the threshold.
    // Ensures the comparison correctly rejects insufficient values.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(25))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::GreaterThan,
        property_name: "health".to_string(),
        value: Some(VariantSnapshot::Int(50)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn builtin_less_than_succeeds() {
    // LessThan should return true when the actual value is below the threshold.
    // Verifies inverse comparison logic works correctly.
    let agent = create_test_agent(vec![("threat_level", VariantSnapshot::Int(3))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::LessThan,
        property_name: "threat_level".to_string(),
        value: Some(VariantSnapshot::Int(5)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn builtin_targets_world_state() {
    // Preconditions can target WorldState instead of Agent.
    // This tests that the target selector correctly switches to world properties.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![("day_time", VariantSnapshot::Bool(true))]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::WorldState,
        operation: PreconditionOp::Equal,
        property_name: "day_time".to_string(),
        value: Some(VariantSnapshot::Bool(true)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn builtin_cross_type_int_float_comparison() {
    // Comparisons should handle int vs float correctly by converting to common type.
    // Tests cross-type numeric comparison in precondition evaluation.
    let agent = create_test_agent(vec![("value", VariantSnapshot::Int(100))]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::GreaterThan,
        property_name: "value".to_string(),
        value: Some(VariantSnapshot::Float(50.5)),
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn custom_precondition_returns_none() {
    // Custom preconditions cannot be evaluated by evaluate_builtin() and return None.
    // This signals the caller to use the callback channel instead.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let precondition = PreconditionSpec::Custom {
        callable_id: 42,
        dependent_object_ids: vec![],
    };

    let result = precondition.evaluate_builtin(&agent, &world);
    assert_eq!(result, None);
}

#[test]
fn world_snapshot_with_objects() {
    // World snapshots can contain SimObjectData in addition to properties.
    // Verifies that objects are correctly stored and accessible in the snapshot.
    let enemy = create_sim_object(
        "enemy_1",
        vec!["enemy", "mobile"],
        vec![("health", VariantSnapshot::Int(50))],
    );

    let world = create_test_world(
        vec![("time", VariantSnapshot::Float(12.5))],
        vec![("enemy_1", enemy)],
    );

    assert_eq!(world.properties.len(), 1);
    assert_eq!(world.objects.len(), 1);
    assert!(world.objects.contains_key("enemy_1"));

    let enemy_obj = &world.objects["enemy_1"];
    assert_eq!(enemy_obj.uid, "enemy_1");
    assert_eq!(enemy_obj.groups.len(), 2);
    assert!(enemy_obj.groups.contains(&"enemy".to_string()));
}

#[test]
fn blackboard_snapshot_clone_independence() {
    // Cloning a BlackboardSnapshot should create an independent copy.
    // Mutations to the clone must not affect the original (critical for simulation isolation).
    let original = create_test_agent(vec![("score", VariantSnapshot::Int(100))]);

    let mut cloned = original.clone();
    cloned
        .properties
        .insert("score".to_string(), VariantSnapshot::Int(200));

    assert_eq!(
        original.properties.get("score"),
        Some(&VariantSnapshot::Int(100))
    );
    assert_eq!(
        cloned.properties.get("score"),
        Some(&VariantSnapshot::Int(200))
    );
}
