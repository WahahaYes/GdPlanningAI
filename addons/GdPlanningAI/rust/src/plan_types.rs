//! Send-safe action/goal/precondition specs and channel message types.
//!
//! The specs ([`ActionSpec`], [`GoalSpec`], [`PreconditionSpec`]) are what the planner thread
//! sees of the user's GDScript actions and goals: plain data plus integer ids that index the
//! scheduler's per-job callable registry. They are built on the main thread by the scheduler.
//!
//! The channel messages ([`CallbackRequest`] and [`PlannerCallback`]) carry the conversation
//! between the planner thread and the main thread; see [`crate::planner::simulation`].

use crate::precondition::{PreconditionOp, PreconditionTarget};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

/// Process-wide request id counter. Starts at 1 because id 0 is reserved as the
/// "no outstanding request" sentinel in [`PlannerRunResult::Pending`].
static NEXT_REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

/// Generates a globally unique ID for a planner-to-main-thread callback request.
///
/// Ids are unique across all jobs, so a stray answer can never be mistaken for another
/// request's.
pub fn next_request_id() -> usize {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

/// Send-safe mirror of [`crate::precondition::PreconditionHandler`].
///
/// Builtin operations carry their data directly and can be evaluated
/// without any channel round-trip. Custom callbacks store
/// a `callable_id` that the main thread resolves via the callable registry.
///
/// Implements `Hash`/`Eq` so specs can key the discovery caches and take part in branch
/// fingerprints.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PreconditionSpec {
    /// A comparison the engine evaluates itself against the snapshots.
    Builtin {
        /// Which blackboard (or group of world objects) the property is read from.
        target: PreconditionTarget,
        /// The comparison to perform.
        operation: PreconditionOp,
        /// Property to read on the target (unused for `WorldObjectProxy`, which carries its
        /// own property name).
        property_name: String,
        /// Right-hand side of the comparison, if the operation takes one.
        value: Option<VariantSnapshot>,
    },
    /// A GDScript callable evaluated on the main thread via a callback.
    Custom {
        /// Index into the job's callable registry.
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
    ///
    /// Numeric comparisons treat ints and floats alike and use a 1e-4 tolerance. A missing
    /// property compares as nil; `WorldObjectProxy` targets hold if any object in the group
    /// satisfies the comparison.
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
                let result = match target {
                    PreconditionTarget::Agent => {
                        eval_builtin_on_snapshot(operation, property_name, value.as_ref(), agent)
                    }
                    PreconditionTarget::WorldState => {
                        eval_builtin_on_snapshot(operation, property_name, value.as_ref(), world)
                    }
                    PreconditionTarget::WorldObjectProxy { group, property } => {
                        // Evaluate against all world objects in the given group
                        // Returns true if ANY object in the group satisfies the condition
                        world.objects.iter().any(|(_, obj)| {
                            if obj.groups.contains(group) {
                                eval_builtin_on_snapshot(
                                    operation,
                                    property,
                                    value.as_ref(),
                                    &BlackboardSnapshot {
                                        properties: obj.properties.clone(),
                                        objects: HashMap::new(),
                                    },
                                )
                            } else {
                                false
                            }
                        })
                    }
                };
                Some(result)
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

impl std::fmt::Display for PreconditionSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Builtin {
                target,
                operation,
                property_name,
                value,
            } => {
                let target_str = match target {
                    PreconditionTarget::Agent => "Agent",
                    PreconditionTarget::WorldState => "World",
                    PreconditionTarget::WorldObjectProxy { .. } => "WorldObjectProxy",
                };
                let op_str = match operation {
                    PreconditionOp::HasProperty => "has",
                    PreconditionOp::Equal => "==",
                    PreconditionOp::NotEqual => "!=",
                    PreconditionOp::GreaterThan => ">",
                    PreconditionOp::GreaterThanOrEqual => ">=",
                    PreconditionOp::LessThan => "<",
                    PreconditionOp::LessThanOrEqual => "<=",
                    PreconditionOp::CustomCallback => "custom",
                };
                if let Some(v) = value {
                    write!(f, "{}.{} {} {:?}", target_str, property_name, op_str, v)
                } else {
                    write!(f, "{}.{} {}", target_str, property_name, op_str)
                }
            }
            Self::Custom { callable_id, .. } => write!(f, "Custom({})", callable_id),
        }
    }
}

/// Evaluate a single builtin precondition against a [`BlackboardSnapshot`].
/// `HasProperty` ignores `value`; every other operation goes through [`snap_compare_all`].
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

/// Equality check mirroring the main-thread `PreconditionHandler` semantics for non-numeric
/// values: nil equals an empty string, ints and floats compare numerically, and other types
/// compare only within the same variant. A missing property is treated as nil.
fn snap_equal(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
) -> bool {
    let prop = source
        .properties
        .get(property_name)
        .unwrap_or(&VariantSnapshot::Nil);
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

/// Applies `operation` to a property and a comparison value. Both are compared as numbers when
/// possible (with a 1e-4 tolerance); otherwise only `Equal` and `NotEqual` are meaningful and
/// the rest evaluate to false.
fn snap_compare_all(
    source: &BlackboardSnapshot,
    property_name: &str,
    compare_val: Option<&VariantSnapshot>,
    operation: &PreconditionOp,
) -> bool {
    let prop = source
        .properties
        .get(property_name)
        .unwrap_or(&VariantSnapshot::Nil);
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

/// Numeric value of an `Int` or `Float` snapshot; `None` for every other variant.
fn snap_as_f64(v: &VariantSnapshot) -> Option<f64> {
    match v {
        VariantSnapshot::Int(i) => Some(*i as f64),
        VariantSnapshot::Float(bits) => Some(f64::from_bits(*bits)),
        _ => None,
    }
}

/// Send-safe description of one GDScript `Action`, built by the scheduler from the
/// serialized action dictionary.
#[derive(Clone, Debug)]
pub struct ActionSpec {
    /// Display name, used for logging and the debug tree.
    pub name: String,
    /// Registry id of the cost callable; `None` means every use costs `1.0`.
    pub cost_callable_id: Option<usize>,
    /// Registry id of the effect callable; `None` means the action does not change state.
    pub effect_callable_id: Option<usize>,
    /// Conditions that must hold in the state just before the action runs.
    pub preconditions: Vec<PreconditionSpec>,
    /// Conditions on the *initial* state that decide whether the action is considered at all
    /// (for example that the entities it needs still exist).
    pub validity_checks: Vec<PreconditionSpec>,
    /// Symbolic needs that an earlier action's provisions must meet.
    pub requirements: Vec<RequirementSpec>,
    /// Symbolic offers this action makes to later actions.
    pub provisions: Vec<ProvisionSpec>,
    /// Object instance IDs this action depends on.
    /// Collected from action callables and all preconditions.
    pub dependent_object_ids: Vec<i64>,
}

/// Send-safe description of one GDScript `Goal`.
#[derive(Clone, Debug)]
pub struct GoalSpec {
    /// Display name, used for logging and the debug tree.
    pub name: String,
    /// Value of achieving the goal; the scheduler searches higher rewards first.
    pub reward: f64,
    /// Conditions that must all hold for the goal to be achieved.
    pub desired_state: Vec<PreconditionSpec>,
    /// Position of the goal in the array passed to `submit_plan`; reported back in the plan
    /// as `goal_index` (goals are filtered and reordered before searching).
    pub original_index: usize,
}

/// Sent from planner thread → main thread.
///
/// The main thread runs `callable_id` from the job's registry on the snapshots in `kind` and
/// answers on `response_tx` with the same `request_id`.
pub struct CallbackRequest {
    /// Id the answer is matched against; unique process-wide (see [`next_request_id`]).
    pub request_id: usize,
    /// Index into the job's callable registry.
    pub callable_id: usize,
    /// Which callable is being run and the state it runs against.
    pub kind: CallbackKind,
    /// Bindings to inject into the agent blackboard before calling the GDScript callable.
    pub bindings: Vec<(String, Vec<VariantSnapshot>)>,
    /// Where to send the answer: the requesting engine's receiver.
    pub response_tx: Sender<PlannerCallback>,
}

/// Result of a callback processed by the main thread.
#[derive(Debug)]
pub struct PlannerCallback {
    /// Matches [`CallbackRequest::request_id`].
    pub request_id: usize,
    /// The value the callable produced, in Send-safe form.
    pub response: CallbackResponse,
}

/// The result of an engine execution step.
#[derive(Debug, Clone)]
pub enum PlannerRunResult {
    /// Planning reached a terminal state (success or total failure). `None` means the job was
    /// cancelled; a search that found nothing yields `Some` of a failed result.
    Complete(Option<crate::plan_tree::PlanResult>),
    /// Planning yielded without a terminal result.
    ///
    /// `Pending(0)` is the budget/time-slice sentinel: the iteration
    /// budget or wall-clock slice elapsed and the search is resumable
    /// unconditionally on the next step. `Pending(id)` with `id > 0`
    /// waits for the GDScript callback with that request id.
    Pending(usize),
}

/// What the main thread should do with the callable.
///
/// The snapshots are converted back into live blackboards on the main thread, which is why
/// the request carries snapshots rather than blackboards.
pub enum CallbackKind {
    /// Call `cost_callable(agent, world)` → return `Float(f64)`.
    /// Bindings are pre-injected into `agent` before the call.
    GetCost {
        /// Agent state the cost is computed against.
        agent: BlackboardSnapshot,
        /// World state passed alongside.
        world: BlackboardSnapshot,
    },
    /// Call `effect_callable(agent, world)` → return updated snapshots.
    /// Bindings are pre-injected into `agent` before the call.
    ApplyEffect {
        /// State the effect is applied to; the callable mutates a rebuilt copy.
        agent: BlackboardSnapshot,
        /// World state passed alongside.
        world: BlackboardSnapshot,
    },
    /// Call `eval_callable(agent, world)` → return `Bool(bool)`.
    /// Bindings are pre-injected into `agent` before the call.
    EvalCustomPrecond {
        /// State the precondition is evaluated against.
        agent: BlackboardSnapshot,
        /// World state passed alongside.
        world: BlackboardSnapshot,
    },
}

/// Sent from main thread → planner thread.
#[derive(Debug, Clone)]
pub enum CallbackResponse {
    /// The cost returned by a [`CallbackKind::GetCost`] callable (infinity if it returned a
    /// non-number).
    Float(f64),
    /// The result of a [`CallbackKind::EvalCustomPrecond`] callable (false if it returned a
    /// non-bool).
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
