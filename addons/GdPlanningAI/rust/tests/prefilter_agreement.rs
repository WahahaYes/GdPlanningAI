//! Pre-filter/sim agreement on builtin preconditions.
//!
//! The `submit_plan` goal pre-filter and the planner both evaluate builtins through
//! `evaluate_builtin`, so these edge cases pin the one shared semantics: presence-based
//! `HasProperty`, nil/empty-string equality, and 1e-4 float tolerance.

mod common;

use common::create_test_agent;
use gdplanningai_rust::plan_types::PreconditionSpec;
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::VariantSnapshot;

fn agent_precondition(
    operation: PreconditionOp,
    property_name: &str,
    value: Option<VariantSnapshot>,
) -> PreconditionSpec {
    PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation,
        property_name: property_name.to_string(),
        value,
    }
}

#[test]
fn has_property_no_value_holds_when_key_present() {
    // HasProperty ignores `value`: a present key satisfies it even with no comparison value.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(80))]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(PreconditionOp::HasProperty, "health", None)
        .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn has_property_no_value_fails_when_key_missing() {
    // A missing key fails HasProperty regardless of the absent value.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(PreconditionOp::HasProperty, "health", None)
        .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn has_property_holds_for_nil_valued_key() {
    // Presence is key-based: a stored nil still counts as having the property.
    let agent = create_test_agent(vec![("flag", VariantSnapshot::Nil)]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(PreconditionOp::HasProperty, "flag", None)
        .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn equal_missing_property_no_value_is_nil_equal() {
    // A missing property reads as nil, and a missing value compares as nil: nil equals nil.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let result =
        agent_precondition(PreconditionOp::Equal, "missing", None).evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn equal_missing_property_explicit_nil_is_true() {
    // Missing property against an explicit nil comparison value holds.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(PreconditionOp::Equal, "missing", Some(VariantSnapshot::Nil))
        .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn equal_nil_matches_empty_string() {
    // Nil and an empty string compare equal in either operand position.
    let agent = create_test_agent(vec![
        ("blank", VariantSnapshot::Str(String::new())),
        ("nothing", VariantSnapshot::Nil),
    ]);
    let world = create_test_agent(vec![]);

    let nil_vs_empty = agent_precondition(
        PreconditionOp::Equal,
        "nothing",
        Some(VariantSnapshot::Str(String::new())),
    )
    .evaluate_builtin(&agent, &world);
    assert_eq!(nil_vs_empty, Some(true));

    let empty_vs_nil =
        agent_precondition(PreconditionOp::Equal, "blank", Some(VariantSnapshot::Nil))
            .evaluate_builtin(&agent, &world);
    assert_eq!(empty_vs_nil, Some(true));
}

#[test]
fn not_equal_nil_vs_nil_is_false() {
    // Nil equals nil, so NotEqual on two nils fails.
    let agent = create_test_agent(vec![]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(PreconditionOp::NotEqual, "missing", None)
        .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn equal_float_within_tolerance_holds() {
    // Floats within 1e-4 compare equal (here the gap is 5e-5).
    let agent = create_test_agent(vec![("level", VariantSnapshot::Float(1.0f64.to_bits()))]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(
        PreconditionOp::Equal,
        "level",
        Some(VariantSnapshot::Float(1.00005f64.to_bits())),
    )
    .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn equal_float_outside_tolerance_fails() {
    // Floats 1e-3 apart are genuinely different.
    let agent = create_test_agent(vec![("level", VariantSnapshot::Float(1.0f64.to_bits()))]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(
        PreconditionOp::Equal,
        "level",
        Some(VariantSnapshot::Float(1.001f64.to_bits())),
    )
    .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn greater_than_inside_tolerance_band_fails() {
    // Strict comparisons carry the 1e-4 band: 5e-5 above the threshold is not greater.
    let agent = create_test_agent(vec![(
        "score",
        VariantSnapshot::Float(50.00005f64.to_bits()),
    )]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(
        PreconditionOp::GreaterThan,
        "score",
        Some(VariantSnapshot::Float(50.0f64.to_bits())),
    )
    .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(false));
}

#[test]
fn greater_than_or_equal_inside_tolerance_band_holds() {
    // 5e-5 below the threshold still satisfies GreaterThanOrEqual within tolerance.
    let agent = create_test_agent(vec![(
        "score",
        VariantSnapshot::Float(49.99995f64.to_bits()),
    )]);
    let world = create_test_agent(vec![]);

    let result = agent_precondition(
        PreconditionOp::GreaterThanOrEqual,
        "score",
        Some(VariantSnapshot::Float(50.0f64.to_bits())),
    )
    .evaluate_builtin(&agent, &world);
    assert_eq!(result, Some(true));
}

#[test]
fn custom_precondition_defers_to_live_handler() {
    // Custom specs answer None so the pre-filter falls back to the live handler's callables.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(80))]);
    let world = create_test_agent(vec![]);

    let spec = PreconditionSpec::Custom {
        callable_id: 0,
        dependent_object_ids: vec![],
    };
    assert_eq!(spec.evaluate_builtin(&agent, &world), None);
}
