//! Send-safe action/goal/precondition specs and channel message types.

use crate::precondition::{PreconditionOp, PreconditionTarget};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

static NEXT_REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

/// Generates a globally unique ID for a planner-to-main-thread callback request.
pub fn next_request_id() -> usize {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

/// Send-safe mirror of [`crate::precondition::PreconditionHandler`].
///
/// Builtin operations carry their data directly and can be evaluated
/// without any channel round-trip. Custom callbacks store
/// a `callable_id` that the main thread resolves via the callable registry.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PreconditionSpec {
    Builtin {
        target: PreconditionTarget,
        operation: PreconditionOp,
        property_name: String,
        value: Option<VariantSnapshot>,
    },
    Custom {
        callable_id: usize,
        /// Object instance IDs this precondition depends on for validity checking.
        dependent_object_ids: Vec<i64>,
    },
}

impl PreconditionSpec {
    /// Evaluate a builtin precondition directly against snapshots.
    ///
    /// Returns `None` for `Custom` variants — the caller must use the
    /// callback channel instead.
    pub fn evaluate_builtin(
        &self,
        agent: &BlackboardSnapshot,
        world: &BlackboardSnapshot,
    ) -> Option<bool> {
        match self {
            Self::Builtin {
                target,
                operation,
                property_name,
                value,
            } => {
                let source = match target {
                    PreconditionTarget::Agent => agent,
                    PreconditionTarget::WorldState => world,
                };
                Some(eval_builtin_on_snapshot(
                    operation,
                    property_name,
                    value.as_ref(),
                    source,
                ))
            }
            Self::Custom { .. } => None,
        }
    }

    /// Returns the callable ID if this is a `Custom` variant.
    pub fn callable_id(&self) -> Option<usize> {
        match self {
            Self::Custom { callable_id, .. } => Some(*callable_id),
            _ => None,
        }
    }

    /// Returns the property name if this is a `Builtin` variant.
    pub fn property_name(&self) -> Option<&str> {
        match self {
            Self::Builtin { property_name, .. } => Some(property_name.as_str()),
            _ => None,
        }
    }

    /// Returns the dependent object IDs if this is a `Custom` variant.
    pub fn dependent_object_ids(&self) -> &[i64] {
        match self {
            Self::Custom {
                dependent_object_ids,
                ..
            } => dependent_object_ids.as_slice(),
            _ => &[],
        }
    }
}

/// Evaluate a single builtin precondition against a [`BlackboardSnapshot`].
fn eval_builtin_on_snapshot(
    operation: &PreconditionOp,
    property_name: &str,
    value: Option<&VariantSnapshot>,
    source: &BlackboardSnapshot,
) -> bool {
    match operation {
        PreconditionOp::HasProperty => source.properties.contains_key(property_name),
        _ => snap_compare_all(source, property_name, value, operation),
    }
}

/// Equality check mirroring `PreconditionHandler::evaluate_equal`.
fn snap_equal(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
) -> bool {
    let prop = source.properties.get(property_name).unwrap_or(&VariantSnapshot::Nil);
    let cmp = match compare_val {
        Some(v) => v,
        None => &VariantSnapshot::Nil,
    };

    match (prop, cmp) {
        (VariantSnapshot::Nil, VariantSnapshot::Nil) => true,
        (VariantSnapshot::Nil, VariantSnapshot::Str(s)) if s.is_empty() => true,
        (VariantSnapshot::Str(s), VariantSnapshot::Nil) if s.is_empty() => true,
        (VariantSnapshot::Bool(a), VariantSnapshot::Bool(b)) => a == b,
        (VariantSnapshot::Str(a), VariantSnapshot::Str(b)) => a == b,
        (VariantSnapshot::Int(a), VariantSnapshot::Int(b)) => a == b,
        (VariantSnapshot::ObjectRef(a), VariantSnapshot::ObjectRef(b)) => a == b,
        // Numeric: cross-compare int/float
        (p, c) => match (snap_as_f64(p), snap_as_f64(c)) {
            (Some(pn), Some(cn)) => (pn - cn).abs() < 1e-4,
            _ => false,
        },
    }
}

/// Numeric comparison handling all operators.
fn snap_compare_all(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
    operation: &PreconditionOp,
) -> bool {
    let prop = source.properties.get(property_name).unwrap_or(&VariantSnapshot::Nil);
    let p_num = snap_as_f64(prop);
    let c_num = compare_val.and_then(snap_as_f64);

    if let (Some(p), Some(c)) = (p_num, c_num) {
        match operation {
            PreconditionOp::Equal => (p - c).abs() < 1e-4,
            PreconditionOp::NotEqual => (p - c).abs() >= 1e-4,
            PreconditionOp::GreaterThan => p > c + 1e-4,
            PreconditionOp::GreaterThanOrEqual => p >= c - 1e-4,
            PreconditionOp::LessThan => p < c - 1e-4,
            PreconditionOp::LessThanOrEqual => p <= c + 1e-4,
            _ => false,
        }
    } else {
        // Fallback to basic equality if not numeric
        match operation {
            PreconditionOp::Equal => snap_equal(source, property_name, compare_val),
            PreconditionOp::NotEqual => !snap_equal(source, property_name, compare_val),
            _ => false,
        }
    }
}

fn snap_as_f64(v: &VariantSnapshot) -> Option<f64> {
    match v {
        VariantSnapshot::Int(i) => Some(*i as f64),
        VariantSnapshot::Float(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

/// Policy for when to re-simulate action effects during planning.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RipplePolicy {
    Always,
    OnRequirement,
    Never,
}

/// Send-safe mirror of [`crate::action::ActionData`].
#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub name: String,
    pub cost_callable_id: Option<usize>,
    pub effect_callable_id: Option<usize>,
    pub preconditions: Vec<PreconditionSpec>,
    pub validity_checks: Vec<PreconditionSpec>,
    pub requirements: Vec<RequirementSpec>,
    pub provisions: Vec<ProvisionSpec>,
    /// Object instance IDs this action depends on.
    /// Collected from action callables and all preconditions.
    pub dependent_object_ids: Vec<i64>,
}

/// Send-safe mirror of [`crate::goal::GoalData`].
#[derive(Clone, Debug)]
pub struct GoalSpec {
    pub name: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionSpec>,
    pub original_index: usize,
}

/// Identifies the specific type of simulation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RequestKind {
    Precondition,
    Cost,
    Effect,
}

/// Sent from planner thread → main thread.
pub struct CallbackRequest {
    pub request_id: usize,
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub response_tx: Sender<PlannerCallback>,
}

/// Result of a callback processed by the main thread.
#[derive(Debug)]
pub struct PlannerCallback {
    pub request_id: usize,
    pub response: CallbackResponse,
}

/// The result of an engine execution step.
#[derive(Debug, Clone)]
pub enum PlannerRunResult {
    /// Planning reached a terminal state (success or total failure).
    Complete(Option<crate::plan_tree::PlanResult>),
    /// Planning is paused waiting for a GDScript callback.
    Pending(usize),
}

/// What the main thread should do with the callable.
pub enum CallbackKind {
    /// Call `cost_callable(agent, world, provisions, bindings)` → return `Float(f64)`.
    GetCost {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
    /// Call `effect_callable(agent, world, provisions, bindings)` → return updated snapshots.
    ApplyEffect {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
    /// Call `eval_callable(agent, world, provisions, bindings)` → return `Bool(bool)`.
    EvalCustomPrecond {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
}

/// Sent from main thread → planner thread.
#[derive(Debug, Clone)]
pub enum CallbackResponse {
    Float(f64),
    Bool(bool),
    /// Mutated snapshots after `ApplyEffect`.
    UpdatedSnapshots(BlackboardSnapshot, BlackboardSnapshot),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precondition::{PreconditionOp, PreconditionTarget};
    use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
    use std::collections::HashMap;

    fn make_agent_snapshot() -> BlackboardSnapshot {
        let mut properties = HashMap::new();
        properties.insert("health".to_string(), VariantSnapshot::Int(80));
        properties.insert(
            "stamina".to_string(),
            VariantSnapshot::Float(65.5f64.to_bits()),
        );
        properties.insert("name".to_string(), VariantSnapshot::Str("Hero".to_string()));
        properties.insert("alive".to_string(), VariantSnapshot::Bool(true));

        BlackboardSnapshot {
            properties,
            objects: HashMap::new(),
        }
    }

    fn make_world_snapshot() -> BlackboardSnapshot {
        let mut properties = HashMap::new();
        properties.insert(
            "time".to_string(),
            VariantSnapshot::Float(123.45f64.to_bits()),
        );
        properties.insert("enemy_count".to_string(), VariantSnapshot::Int(5));

        BlackboardSnapshot {
            properties,
            objects: HashMap::new(),
        }
    }

    #[test]
    fn has_property_true_when_exists() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "health".to_string(),
            value: None,
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn has_property_false_when_missing() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "nonexistent".to_string(),
            value: None,
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(false));
    }

    #[test]
    fn equal_integer_matches() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(80)),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn equal_integer_fails_on_mismatch() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(100)),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(false));
    }

    #[test]
    fn greater_than_comparison_works() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(50)),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn less_than_comparison_works() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(100)),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn cross_type_int_float_comparison() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Float(79.5f64.to_bits())),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn targets_world_state_correctly() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::WorldState,
            operation: PreconditionOp::Equal,
            property_name: "enemy_count".to_string(),
            value: Some(VariantSnapshot::Int(5)),
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), Some(true));
    }

    #[test]
    fn custom_callback_returns_none() {
        let spec = PreconditionSpec::Custom {
            callable_id: 0,
            dependent_object_ids: vec![],
        };

        let agent = make_agent_snapshot();
        let world = make_world_snapshot();

        assert_eq!(spec.evaluate_builtin(&agent, &world), None);
    }

    #[test]
    fn callable_id_accessor() {
        let spec = PreconditionSpec::Custom {
            callable_id: 42,
            dependent_object_ids: vec![],
        };

        assert_eq!(spec.callable_id(), Some(42));
    }

    #[test]
    fn dependent_object_ids_accessor() {
        let spec = PreconditionSpec::Custom {
            callable_id: 0,
            dependent_object_ids: vec![123, 456, 789],
        };

        let deps = spec.dependent_object_ids();
        assert_eq!(deps, &[123, 456, 789]);
    }

    #[test]
    fn builtin_has_empty_dependencies() {
        let spec = PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::HasProperty,
            property_name: "test".to_string(),
            value: None,
        };

        assert_eq!(spec.dependent_object_ids().len(), 0);
    }
}
