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
            PreconditionSpec::Custom { callable_id, .. } => registry.get(*callable_id).map(|c| Self {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::CustomCallback,
                property_name: String::new(),
                value: None,
                eval_callable: Some(c.clone()),
            }),
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

        let source = match self.target {
            PreconditionTarget::Agent => agent_state,
            PreconditionTarget::WorldState => world_state,
        };

        match &self.operation {
            PreconditionOp::HasProperty => source
                .bind()
                .has_property(GString::from(&self.property_name)),
            PreconditionOp::Equal => self.evaluate_equal(source),
            PreconditionOp::NotEqual => !self.evaluate_equal(source),
            PreconditionOp::GreaterThan => self.evaluate_greater_than(source),
            PreconditionOp::GreaterThanOrEqual => {
                self.evaluate_greater_than(source) || self.evaluate_equal(source)
            }
            PreconditionOp::LessThan => self.evaluate_less_than(source),
            PreconditionOp::LessThanOrEqual => {
                self.evaluate_less_than(source) || self.evaluate_equal(source)
            }
            PreconditionOp::CustomCallback => false, // Handled above
        }
    }

    /// Checks equality between the named property and [`self.value`].
    ///
    /// Tries exact `Variant` equality first, then numeric comparison with
    /// epsilon tolerance, then string, then bool. Returns `false` if the
    /// property is absent or no comparison succeeds.
    fn evaluate_equal(&self, source: &Gd<GdPAIBlackboard>) -> bool {
        let prop = source
            .bind()
            .get_property(GString::from(&self.property_name));
        if prop.is_nil() {
            return false;
        }

        let compare_val = match &self.value {
            Some(v) => v,
            None => return false,
        };

        if &prop == compare_val {
            return true;
        }

        if let (Some(p_num), Some(c_num)) =
            (prop.try_to::<f64>().ok(), compare_val.try_to::<f64>().ok())
        {
            return (p_num - c_num).abs() < f64::EPSILON;
        }

        if let (Ok(p_str), Ok(c_str)) = (prop.try_to::<String>(), compare_val.try_to::<String>()) {
            return p_str == c_str;
        }

        if let (Ok(p_bool), Ok(c_bool)) = (prop.try_to::<bool>(), compare_val.try_to::<bool>()) {
            return p_bool == c_bool;
        }

        false
    }

    /// Retrieves a property as `f64`, accepting both `float` and `int` variants.
    /// Returns `None` if the property is absent or not numeric.
    fn get_numeric_property(source: &Gd<GdPAIBlackboard>, property_name: &str) -> Option<f64> {
        let prop = source.bind().get_property(property_name.into());
        if prop.is_nil() {
            return None;
        }
        if let Ok(f) = prop.try_to::<f64>() {
            return Some(f);
        }
        if let Ok(i) = prop.try_to::<i64>() {
            return Some(i as f64);
        }
        None
    }

    /// Returns `true` if the named property is numerically greater than [`self.value`].
    fn evaluate_greater_than(&self, source: &Gd<GdPAIBlackboard>) -> bool {
        let prop_num = Self::get_numeric_property(source, &self.property_name);
        let compare_num = self.value.as_ref().and_then(|v| {
            v.try_to::<f64>()
                .ok()
                .or_else(|| v.try_to::<i64>().ok().map(|i| i as f64))
        });

        match (prop_num, compare_num) {
            (Some(p), Some(c)) => p > c,
            _ => false,
        }
    }

    /// Returns `true` if the named property is numerically less than [`self.value`].
    fn evaluate_less_than(&self, source: &Gd<GdPAIBlackboard>) -> bool {
        let prop_num = Self::get_numeric_property(source, &self.property_name);
        let compare_num = self.value.as_ref().and_then(|v| {
            v.try_to::<f64>()
                .ok()
                .or_else(|| v.try_to::<i64>().ok().map(|i| i as f64))
        });

        match (prop_num, compare_num) {
            (Some(p), Some(c)) => p < c,
            _ => false,
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
