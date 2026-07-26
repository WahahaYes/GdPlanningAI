//! Precondition handling for the planning engine.
//!
//! Built-in operations (has_property, equal, greater_than, etc.) are handled
//! in Rust for performance. Custom preconditions use Callables for GDScript evaluation.

use super::gdpai_blackboard::GdPAIBlackboard;
use crate::plan_types::PreconditionSpec;
use crate::snapshot::VariantSnapshot;
use godot::prelude::*;

/// Handler for precondition evaluation with built-in or custom operations.
#[derive(Clone, Debug)]
pub struct PreconditionHandler {
    /// Where to evaluate the precondition
    pub target: PreconditionTarget,
    /// Operation to perform
    pub operation: PreconditionOp,
    /// Property name to check
    pub property_name: String,
    /// Value to compare against (for comparison operations)
    pub value: Option<Variant>,
    /// Callable for custom evaluation (if operation is CustomCallback)
    pub eval_callable: Option<Callable>,
}

/// Where to evaluate a precondition.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PreconditionTarget {
    Agent,
    WorldState,
    /// Evaluate against properties of world objects in a specific group.
    /// The planner iterates all SimObjectProxy snapshots in the world state
    /// that belong to the given group and checks the property on each.
    WorldObjectProxy {
        group: String,
        property: String,
    },
}

/// Types of precondition operations.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum PreconditionOp {
    HasProperty,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    CustomCallback,
}

impl PreconditionHandler {
    /// Creates a handler from a Godot dictionary.
    ///
    /// Expected keys: `target`, `operation`, `property_name`, `value`,
    /// `is_satisfied`, `eval_callable`.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let target = dict
            .get("target")
            .and_then(|v| v.try_to::<String>().ok())
            .map(|s| match s.to_lowercase().as_str() {
                "world_state" => PreconditionTarget::WorldState,
                "world_object_proxy" => {
                    let group = dict
                        .get("group")
                        .and_then(|v| v.try_to::<String>().ok())
                        .unwrap_or_default();
                    let property = dict
                        .get("property")
                        .and_then(|v| v.try_to::<String>().ok())
                        .unwrap_or_default();
                    PreconditionTarget::WorldObjectProxy { group, property }
                }
                _ => PreconditionTarget::Agent,
            })
            .unwrap_or(PreconditionTarget::Agent);

        let operation = dict
            .get("operation")
            .and_then(|v| v.try_to::<String>().ok())
            .map(|s| Self::parse_operation(&s))
            .unwrap_or(PreconditionOp::HasProperty);

        let property_name = dict
            .get("property_name")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        let value = dict.get("value");

        let eval_callable = dict
            .get("eval_callable")
            .and_then(|v| v.try_to::<Callable>().ok());

        Some(Self {
            target,
            operation,
            property_name,
            value,
            eval_callable,
        })
    }

    /// Reconstructs a handler from a send-safe [`PreconditionSpec`] and the
    /// callable registry it indexes into. This lets the existing synchronous
    /// evaluation logic be reused on the main thread.
    pub fn from_spec(spec: &PreconditionSpec, registry: &[Callable]) -> Option<Self> {
        match spec {
            PreconditionSpec::Builtin {
                target,
                operation,
                property_name,
                value,
            } => Some(Self {
                target: target.clone(),
                operation: operation.clone(),
                property_name: property_name.clone(),
                value: value.as_ref().map(VariantSnapshot::to_variant),
                eval_callable: None,
            }),
            PreconditionSpec::Custom { callable_id, .. } => {
                registry.get(*callable_id).map(|c| Self {
                    target: PreconditionTarget::Agent,
                    operation: PreconditionOp::CustomCallback,
                    property_name: String::new(),
                    value: None,
                    eval_callable: Some(c.clone()),
                })
            }
        }
    }

    /// Maps an operation name string from the GDScript bridge to a [`PreconditionOp`] variant.
    /// Unrecognised strings fall back to [`PreconditionOp::HasProperty`].
    pub fn parse_operation(s: &str) -> PreconditionOp {
        match s.to_lowercase().as_str() {
            "has_property" => PreconditionOp::HasProperty,
            "equal" => PreconditionOp::Equal,
            "not_equal" => PreconditionOp::NotEqual,
            "greater_than" => PreconditionOp::GreaterThan,
            "greater_than_or_equal" => PreconditionOp::GreaterThanOrEqual,
            "less_than" => PreconditionOp::LessThan,
            "less_than_or_equal" => PreconditionOp::LessThanOrEqual,
            "custom_callback" => PreconditionOp::CustomCallback,
            _ => {
                log_warn!(
                    "Unrecognised precondition operation '{}'; defaulting to HasProperty",
                    s
                );
                PreconditionOp::HasProperty
            }
        }
    }

    /// Evaluates this precondition against the given states.
    pub fn evaluate(
        &self,
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        if self.operation == PreconditionOp::CustomCallback {
            if let Some(callable) = &self.eval_callable {
                let result = callable.call(&[agent_state.to_variant(), world_state.to_variant()]);
                return result.try_to::<bool>().unwrap_or(false);
            }
            log_warn!(
                "CustomCallback precondition on '{}' has no callable; evaluating as false",
                self.property_name
            );
            return false;
        }

        match self.target {
            PreconditionTarget::Agent => {
                let source = agent_state;
                Self::evaluate_on_blackboard(
                    source,
                    &self.operation,
                    &self.property_name,
                    &self.value,
                )
            }
            PreconditionTarget::WorldState => {
                let source = world_state;
                Self::evaluate_on_blackboard(
                    source,
                    &self.operation,
                    &self.property_name,
                    &self.value,
                )
            }
            PreconditionTarget::WorldObjectProxy {
                ref group,
                ref property,
            } => {
                // Iterate all objects in the world state that belong to the specified group
                let world_bind = world_state.bind();
                let objects = world_bind.get_proxies_in_group(GString::from(group.as_str()));
                for obj in objects.iter_shared() {
                    let prop = obj.bind().get_property(GString::from(property.as_str()));
                    if !prop.is_nil() {
                        let val = match &self.value {
                            Some(v) => v.to_variant(),
                            None => Variant::nil(),
                        };
                        // Compare prop with val based on operation
                        if Self::compare_variants(&prop, &val, &self.operation) {
                            return true;
                        }
                    }
                }
                false
            }
        }
    }

    /// Evaluate a builtin operation on a GdPAIBlackboard source.
    fn evaluate_on_blackboard(
        source: &Gd<GdPAIBlackboard>,
        op: &PreconditionOp,
        property_name: &str,
        value: &Option<Variant>,
    ) -> bool {
        let bind = source.bind();
        let prop = bind.get_property(GString::from(property_name));
        if prop.is_nil() {
            return false;
        }

        let compare_val = match value {
            Some(v) => v,
            None => return false,
        };

        match op {
            PreconditionOp::HasProperty => !prop.is_nil(),
            PreconditionOp::Equal => {
                if prop == *compare_val {
                    return true;
                }
                // Numeric comparison with epsilon
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), compare_val.try_to::<f64>()) {
                    return (p - v).abs() < f64::EPSILON;
                }
                false
            }
            PreconditionOp::NotEqual => !Self::compare_variants_equal(&prop, compare_val),
            PreconditionOp::GreaterThan => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), compare_val.try_to::<f64>()) {
                    return p > v;
                }
                false
            }
            PreconditionOp::GreaterThanOrEqual => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), compare_val.try_to::<f64>()) {
                    return p >= v;
                }
                false
            }
            PreconditionOp::LessThan => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), compare_val.try_to::<f64>()) {
                    return p < v;
                }
                false
            }
            PreconditionOp::LessThanOrEqual => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), compare_val.try_to::<f64>()) {
                    return p <= v;
                }
                false
            }
            PreconditionOp::CustomCallback => false,
        }
    }

    /// Compare two Variants for equality (numeric with epsilon).
    fn compare_variants_equal(prop: &Variant, val: &Variant) -> bool {
        if *prop == *val {
            return true;
        }
        if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), val.try_to::<f64>()) {
            return (p - v).abs() < f64::EPSILON;
        }
        false
    }

    /// Compare two Variants using the given operation.
    fn compare_variants(prop: &Variant, val: &Variant, op: &PreconditionOp) -> bool {
        match op {
            PreconditionOp::HasProperty => !prop.is_nil(),
            PreconditionOp::Equal => Self::compare_variants_equal(prop, val),
            PreconditionOp::NotEqual => !Self::compare_variants_equal(prop, val),
            PreconditionOp::GreaterThan => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), val.try_to::<f64>()) {
                    return p > v;
                }
                false
            }
            PreconditionOp::GreaterThanOrEqual => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), val.try_to::<f64>()) {
                    return p >= v;
                }
                false
            }
            PreconditionOp::LessThan => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), val.try_to::<f64>()) {
                    return p < v;
                }
                false
            }
            PreconditionOp::LessThanOrEqual => {
                if let (Ok(p), Ok(v)) = (prop.try_to::<f64>(), val.try_to::<f64>()) {
                    return p <= v;
                }
                false
            }
            PreconditionOp::CustomCallback => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_all_known_operations() {
        assert_eq!(
            PreconditionHandler::parse_operation("has_property"),
            PreconditionOp::HasProperty
        );
        assert_eq!(
            PreconditionHandler::parse_operation("equal"),
            PreconditionOp::Equal
        );
        assert_eq!(
            PreconditionHandler::parse_operation("not_equal"),
            PreconditionOp::NotEqual
        );
        assert_eq!(
            PreconditionHandler::parse_operation("greater_than"),
            PreconditionOp::GreaterThan
        );
        assert_eq!(
            PreconditionHandler::parse_operation("greater_than_or_equal"),
            PreconditionOp::GreaterThanOrEqual
        );
        assert_eq!(
            PreconditionHandler::parse_operation("less_than"),
            PreconditionOp::LessThan
        );
        assert_eq!(
            PreconditionHandler::parse_operation("less_than_or_equal"),
            PreconditionOp::LessThanOrEqual
        );
        assert_eq!(
            PreconditionHandler::parse_operation("custom_callback"),
            PreconditionOp::CustomCallback
        );
    }

    #[test]
    fn parse_operation_is_case_insensitive() {
        assert_eq!(
            PreconditionHandler::parse_operation("EQUAL"),
            PreconditionOp::Equal
        );
        assert_eq!(
            PreconditionHandler::parse_operation("Greater_Than"),
            PreconditionOp::GreaterThan
        );
        assert_eq!(
            PreconditionHandler::parse_operation("HAS_PROPERTY"),
            PreconditionOp::HasProperty
        );
        assert_eq!(
            PreconditionHandler::parse_operation("CUSTOM_CALLBACK"),
            PreconditionOp::CustomCallback
        );
    }

    #[test]
    fn parse_unknown_operation_defaults_to_has_property() {
        assert_eq!(
            PreconditionHandler::parse_operation("nonexistent"),
            PreconditionOp::HasProperty
        );
        assert_eq!(
            PreconditionHandler::parse_operation(""),
            PreconditionOp::HasProperty
        );
        assert_eq!(
            PreconditionHandler::parse_operation("gt"),
            PreconditionOp::HasProperty
        );
    }
}
