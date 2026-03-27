//! Send-safe action/goal/precondition specs and channel message types
//! for background planning.

use crate::precondition::{PreconditionOp, PreconditionTarget};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::sync::mpsc::Sender;

// ---------------------------------------------------------------------------
// PreconditionSpec
// ---------------------------------------------------------------------------

/// Send-safe mirror of [`crate::precondition::PreconditionHandler`].
///
/// Builtin operations carry their data directly and can be evaluated on the
/// background thread without any channel round-trip. Custom callbacks store
/// a `callable_id` that the main thread resolves via the callable registry.
#[derive(Clone, Debug)]
pub enum PreconditionSpec {
    Builtin {
        target: PreconditionTarget,
        operation: PreconditionOp,
        property_name: String,
        value: Option<VariantSnapshot>,
    },
    Custom {
        callable_id: usize,
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
            Self::Custom { callable_id } => Some(*callable_id),
            _ => None,
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
        PreconditionOp::Equal => snap_equal(source, property_name, value),
        PreconditionOp::NotEqual => !snap_equal(source, property_name, value),
        PreconditionOp::GreaterThan => snap_compare(source, property_name, value, |p, c| p > c),
        PreconditionOp::GreaterThanOrEqual => {
            snap_compare(source, property_name, value, |p, c| p >= c)
        }
        PreconditionOp::LessThan => snap_compare(source, property_name, value, |p, c| p < c),
        PreconditionOp::LessThanOrEqual => {
            snap_compare(source, property_name, value, |p, c| p <= c)
        }
        PreconditionOp::CustomCallback => false, // should not reach here
    }
}

/// Equality check mirroring `PreconditionHandler::evaluate_equal`.
fn snap_equal(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
) -> bool {
    let prop = match source.properties.get(property_name) {
        Some(v) => v,
        None => return false,
    };
    let cmp = match compare_val {
        Some(v) => v,
        None => return false,
    };
    match (prop, cmp) {
        (VariantSnapshot::Nil, VariantSnapshot::Nil) => true,
        (VariantSnapshot::Bool(a), VariantSnapshot::Bool(b)) => a == b,
        (VariantSnapshot::Str(a), VariantSnapshot::Str(b)) => a == b,
        // Numeric: cross-compare int/float
        (p, c) => match (snap_as_f64(p), snap_as_f64(c)) {
            (Some(pn), Some(cn)) => (pn - cn).abs() < f64::EPSILON,
            _ => false,
        },
    }
}

/// Numeric comparison with a caller-supplied operator.
fn snap_compare(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
    cmp: fn(f64, f64) -> bool,
) -> bool {
    let prop = source.properties.get(property_name);
    let p_num = prop.and_then(snap_as_f64);
    let c_num = compare_val.and_then(snap_as_f64);
    match (p_num, c_num) {
        (Some(p), Some(c)) => cmp(p, c),
        _ => false,
    }
}

fn snap_as_f64(v: &VariantSnapshot) -> Option<f64> {
    match v {
        VariantSnapshot::Int(i) => Some(*i as f64),
        VariantSnapshot::Float(f) => Some(*f),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// ActionSpec / GoalSpec
// ---------------------------------------------------------------------------

/// Send-safe mirror of [`crate::action::ActionData`].
#[derive(Clone, Debug)]
pub struct ActionSpec {
    pub name: String,
    pub cost_callable_id: usize,
    pub effect_callable_id: usize,
    pub preconditions: Vec<PreconditionSpec>,
    pub validity_checks: Vec<PreconditionSpec>,
}

/// Send-safe mirror of [`crate::goal::GoalData`].
#[derive(Clone, Debug)]
pub struct GoalSpec {
    pub name: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionSpec>,
    pub original_index: usize,
}

// ---------------------------------------------------------------------------
// Channel messages
// ---------------------------------------------------------------------------

/// Sent from background thread → main thread.
pub struct CallbackRequest {
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub response_tx: Sender<CallbackResponse>,
}

/// What the main thread should do with the callable.
pub enum CallbackKind {
    /// Call `cost_callable(agent, world)` → return `Float(f64)`.
    GetCost {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
    /// Call `effect_callable(agent, world)` → return updated snapshots.
    ApplyEffect {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
    /// Call `eval_callable(agent, world)` → return `Bool(bool)`.
    EvalCustomPrecond {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
}

/// Sent from main thread → background thread.
pub enum CallbackResponse {
    Float(f64),
    Bool(bool),
    /// Mutated snapshots after `ApplyEffect`.
    UpdatedSnapshots(BlackboardSnapshot, BlackboardSnapshot),
}
