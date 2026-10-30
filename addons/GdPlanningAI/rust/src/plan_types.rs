//! Send-safe action/goal/precondition specs and channel message types.
//!
//! The specs ([`ActionSpec`], [`GoalSpec`], [`PreconditionSpec`]) are what the planner thread
//! sees of the user's GDScript actions and goals: plain data plus integer ids that index the
//! scheduler's per-job callable registry. They are built on the main thread by the scheduler.
//!
//! The channel messages ([`CallbackRequest`] and [`PlannerCallback`]) carry the conversation
//! between the planner thread and the main thread; see [`crate::planner::simulation`].

use crate::effect_ir::EffectOp;
use crate::precondition::{PreconditionOp, PreconditionTarget};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::Sender;

/// Process-wide request id counter. Starts at 1 because id 0 is reserved as the
/// "no outstanding request" sentinel in [`PlannerRunResult::Pending`].
static NEXT_REQUEST_ID: AtomicUsize = AtomicUsize::new(1);

/// Returns a callback request id unique across all jobs.
pub fn next_request_id() -> usize {
    NEXT_REQUEST_ID.fetch_add(1, Ordering::SeqCst)
}

/// Send-safe mirror of [`crate::precondition::PreconditionHandler`]. Builtins carry their
/// data; custom ones carry a `callable_id` resolved on the main thread. Hashable, so specs can
/// key caches and fingerprints.
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
    /// Evaluates a builtin precondition against snapshots; `None` for `Custom` (use a callback).
    /// Numbers compare with a 1e-4 tolerance; group targets hold if any object in the group matches.
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
                                        properties: Arc::clone(&obj.properties),
                                        objects: Arc::new(HashMap::new()),
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

/// Equality for operands that are not both numbers ([`snap_compare_all`] compares those first):
/// nil equals an empty string, other values only within the same variant.
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
        (VariantSnapshot::ObjectRef(a), VariantSnapshot::ObjectRef(b)) => a == b,
        _ => false,
    }
}

/// Applies `operation` numerically (1e-4 tolerance) when possible; otherwise only `Equal` and
/// `NotEqual` can hold.
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
    /// Native effect spec; when `Some`, the planner applies it directly and the callable above
    /// is never registered. `None` means effects run through GDScript as before.
    pub effect_spec: Option<Vec<EffectOp>>,
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

/// Planner → main thread: run `callable_id` on the snapshots in `kind` and answer on
/// `response_tx` with the same `request_id`.
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

/// What the main thread should do with the callable. Carries snapshots, rebuilt into
/// blackboards on the main thread. Snapshots travel as [`Arc`] so the planner thread and
/// the transport boundary share structure instead of cloning whole maps.
pub enum CallbackKind {
    /// Call `cost_callable(agent, world)` → return `Float(f64)`.
    /// Bindings are pre-injected into `agent` before the call.
    GetCost {
        /// Agent state the cost is computed against.
        agent: Arc<BlackboardSnapshot>,
        /// World state passed alongside.
        world: Arc<BlackboardSnapshot>,
    },
    /// Call `effect_callable(agent, world)` → return updated snapshots.
    /// Bindings are pre-injected into `agent` before the call.
    ApplyEffect {
        /// State the effect is applied to; the callable mutates a rebuilt copy.
        agent: Arc<BlackboardSnapshot>,
        /// World state passed alongside.
        world: Arc<BlackboardSnapshot>,
    },
    /// Call `eval_callable(agent, world)` → return `Bool(bool)`.
    /// Bindings are pre-injected into `agent` before the call.
    EvalCustomPrecond {
        /// State the precondition is evaluated against.
        agent: Arc<BlackboardSnapshot>,
        /// World state passed alongside.
        world: Arc<BlackboardSnapshot>,
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
    UpdatedSnapshots(Arc<BlackboardSnapshot>, Arc<BlackboardSnapshot>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::precondition::{PreconditionOp as Op, PreconditionTarget};
    use crate::snapshot::{BlackboardSnapshot, SimObjectData, VariantSnapshot as V};
    use std::collections::HashMap;

    fn float(x: f64) -> V {
        V::Float(x.to_bits())
    }

    fn snapshot(props: &[(&str, V)]) -> BlackboardSnapshot {
        BlackboardSnapshot {
            properties: Arc::new(
                props
                    .iter()
                    .map(|(k, v)| (k.to_string(), v.clone()))
                    .collect(),
            ),
            objects: Arc::new(HashMap::new()),
        }
    }

    fn object(uid: &str, groups: &[&str], hunger_value: i64) -> (String, Arc<SimObjectData>) {
        let data = Arc::new(SimObjectData {
            uid: uid.to_string(),
            groups: groups.iter().map(|g| g.to_string()).collect(),
            properties: Arc::new(HashMap::from([(
                "hunger_value".to_string(),
                V::Int(hunger_value),
            )])),
        });
        (uid.to_string(), data)
    }

    enum Target {
        Agent,
        World,
        Group(&'static str),
    }

    /// `precondition-semantics`: the built-in evaluator, which the planner and the
    /// `submit_plan` goal pre-filter share, against the quick reference in AUTHORING_GUIDE.md.
    /// Tolerance rows sit exactly on the 1e-4 band edge (`zero` vs `tiny`).
    #[test]
    fn builtin_preconditions_follow_the_quick_reference() {
        let agent = snapshot(&[
            ("health", V::Int(80)),
            ("level", float(1.0)),
            ("zero", float(0.0)),
            ("tiny", float(1e-4)),
            ("above", float(50.00005)),
            ("below", float(49.99995)),
            ("nothing", V::Nil),
            ("blank", V::Str(String::new())),
            ("name", V::Str("a".to_string())),
            ("alive", V::Bool(true)),
            ("target", V::ObjectRef(7)),
            ("shared", V::Int(1)),
        ]);
        let mut world = snapshot(&[("shared", V::Int(2))]);
        world.objects = Arc::new(HashMap::from([
            object("1", &["food"], 30),
            object("2", &["food"], 60),
            object("3", &["rock"], 90),
        ]));

        use Target::{Agent, Group, World};
        #[rustfmt::skip]
        let cases: Vec<(Target, &str, Op, Option<V>, bool)> = vec![
            // HasProperty is presence of the key; the value is ignored.
            (Agent, "health", Op::HasProperty, None, true),
            (Agent, "health", Op::HasProperty, Some(V::Int(0)), true),
            (Agent, "nothing", Op::HasProperty, None, true),
            (Agent, "missing", Op::HasProperty, None, false),
            // Numbers compare across int and float, within 1e-4.
            (Agent, "health", Op::Equal, Some(V::Int(80)), true),
            (Agent, "health", Op::Equal, Some(V::Int(81)), false),
            (Agent, "health", Op::Equal, Some(float(80.0)), true),
            (Agent, "level", Op::Equal, Some(float(1.00005)), true),
            (Agent, "level", Op::Equal, Some(float(1.001)), false),
            (Agent, "zero", Op::Equal, Some(float(1e-4)), false),
            (Agent, "health", Op::NotEqual, Some(V::Int(10)), true),
            (Agent, "health", Op::NotEqual, Some(V::Int(80)), false),
            (Agent, "level", Op::NotEqual, Some(float(1.00005)), false),
            (Agent, "zero", Op::NotEqual, Some(float(1e-4)), true),
            (Agent, "health", Op::GreaterThan, Some(V::Int(50)), true),
            (Agent, "health", Op::GreaterThan, Some(V::Int(80)), false),
            (Agent, "health", Op::GreaterThan, Some(float(79.5)), true),
            (Agent, "above", Op::GreaterThan, Some(V::Int(50)), false),
            (Agent, "tiny", Op::GreaterThan, Some(float(0.0)), false),
            (Agent, "health", Op::GreaterThanOrEqual, Some(V::Int(80)), true),
            (Agent, "health", Op::GreaterThanOrEqual, Some(V::Int(81)), false),
            (Agent, "below", Op::GreaterThanOrEqual, Some(V::Int(50)), true),
            (Agent, "zero", Op::GreaterThanOrEqual, Some(float(1e-4)), true),
            (Agent, "health", Op::LessThan, Some(V::Int(100)), true),
            (Agent, "health", Op::LessThan, Some(V::Int(80)), false),
            (Agent, "below", Op::LessThan, Some(V::Int(50)), false),
            (Agent, "zero", Op::LessThan, Some(float(1e-4)), false),
            (Agent, "health", Op::LessThanOrEqual, Some(V::Int(80)), true),
            (Agent, "health", Op::LessThanOrEqual, Some(V::Int(79)), false),
            (Agent, "above", Op::LessThanOrEqual, Some(V::Int(50)), true),
            (Agent, "tiny", Op::LessThanOrEqual, Some(float(0.0)), true),
            // A missing property and a missing value read as nil; nil equals "" both ways.
            (Agent, "missing", Op::Equal, None, true),
            (Agent, "missing", Op::Equal, Some(V::Nil), true),
            (Agent, "missing", Op::NotEqual, None, false),
            (Agent, "nothing", Op::Equal, Some(V::Str(String::new())), true),
            (Agent, "blank", Op::Equal, Some(V::Nil), true),
            (Agent, "nothing", Op::Equal, Some(V::Str("x".to_string())), false),
            (Agent, "name", Op::Equal, Some(V::Nil), false),
            // Other values are equal only within the same variant.
            (Agent, "name", Op::Equal, Some(V::Str("a".to_string())), true),
            (Agent, "name", Op::Equal, Some(V::Str("b".to_string())), false),
            (Agent, "name", Op::NotEqual, Some(V::Str("b".to_string())), true),
            (Agent, "alive", Op::Equal, Some(V::Bool(true)), true),
            (Agent, "alive", Op::Equal, Some(V::Bool(false)), false),
            (Agent, "target", Op::Equal, Some(V::ObjectRef(7)), true),
            (Agent, "target", Op::Equal, Some(V::ObjectRef(8)), false),
            (Agent, "health", Op::Equal, Some(V::Str("80".to_string())), false),
            (Agent, "alive", Op::Equal, Some(V::Str("true".to_string())), false),
            // Orderings hold only between numbers.
            (Agent, "name", Op::GreaterThan, Some(V::Str("A".to_string())), false),
            (Agent, "health", Op::CustomCallback, Some(V::Int(80)), false),
            // Agent and world are separate blackboards.
            (Agent, "shared", Op::Equal, Some(V::Int(1)), true),
            (World, "shared", Op::Equal, Some(V::Int(2)), true),
            (World, "health", Op::HasProperty, None, false),
            // A group check holds if any object in the group matches.
            (Group("food"), "", Op::Equal, Some(V::Int(60)), true),
            (Group("food"), "", Op::GreaterThan, Some(V::Int(70)), false),
            (Group("rock"), "", Op::GreaterThan, Some(V::Int(70)), true),
            (Group("none"), "", Op::HasProperty, None, false),
        ];

        for (target, property, operation, value, want) in cases {
            let target = match target {
                Agent => PreconditionTarget::Agent,
                World => PreconditionTarget::WorldState,
                Group(group) => PreconditionTarget::WorldObjectProxy {
                    group: group.to_string(),
                    property: "hunger_value".to_string(),
                },
            };
            let spec = PreconditionSpec::Builtin {
                target,
                operation,
                property_name: property.to_string(),
                value,
            };
            assert_eq!(spec.evaluate_builtin(&agent, &world), Some(want), "{spec}");
        }

        // Custom preconditions are not evaluated here: the planner asks a callback, the
        // pre-filter the live handler.
        let custom = PreconditionSpec::Custom {
            callable_id: 0,
            dependent_object_ids: vec![],
        };
        assert_eq!(custom.evaluate_builtin(&agent, &world), None);
    }
}
