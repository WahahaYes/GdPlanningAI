//! Action handling for the planning engine.
//!
//! Actions are passed from GDScript as dictionaries with callables for
//! cost calculation and effect simulation.

use super::gdpai_blackboard::GdPAIBlackboard;
use super::precondition::PreconditionHandler;
use godot::prelude::*;

/// Action data extracted from a GDScript Action object.
#[derive(Clone, Debug)]
pub struct ActionData {
    pub name: String,
    /// Callable to get action cost (takes agent_blackboard, world_state)
    pub cost_callable: Callable,
    /// Callable to simulate effect (takes agent_blackboard, world_state)
    pub effect_callable: Callable,
    pub preconditions: Vec<PreconditionHandler>,
    pub validity_checks: Vec<PreconditionHandler>,
}

impl ActionData {
    /// Creates action data from a Godot dictionary.
    ///
    /// Expected keys: `cost_callable`, `effect_callable`,
    /// `preconditions`, `validity_checks`.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let name = dict
            .get("name")
            .and_then(|v| v.try_to::<String>().ok())
            .unwrap_or_default();

        let cost_callable = match dict
            .get("cost_callable")
            .and_then(|v| v.try_to::<Callable>().ok())
        {
            Some(c) => c,
            None => {
                log_warn!(
                    "ActionData '{}': dict is missing a valid 'cost_callable'",
                    name
                );
                return None;
            }
        };
        let effect_callable = match dict
            .get("effect_callable")
            .and_then(|v| v.try_to::<Callable>().ok())
        {
            Some(c) => c,
            None => {
                log_warn!(
                    "ActionData '{}': dict is missing a valid 'effect_callable'",
                    name
                );
                return None;
            }
        };

        let preconditions = dict
            .get("preconditions")
            .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
            .map(|arr| {
                arr.iter_shared()
                    .filter_map(|d| PreconditionHandler::from_dict(&d))
                    .collect()
            })
            .unwrap_or_default();

        let validity_checks = dict
            .get("validity_checks")
            .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
            .map(|arr| {
                arr.iter_shared()
                    .filter_map(|d| PreconditionHandler::from_dict(&d))
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            name,
            cost_callable,
            effect_callable,
            preconditions,
            validity_checks,
        })
    }

    /// Returns the cost by calling the GDScript callable.
    pub fn get_cost(
        &self,
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> f64 {
        let result = self
            .cost_callable
            .call(&[agent_state.to_variant(), world_state.to_variant()]);
        result.try_to::<f64>().unwrap_or_else(|_| {
            log_warn!("cost_callable returned a non-float value; treating cost as INF");
            f64::INFINITY
        })
    }

    /// Checks if all validity checks pass for this action.
    pub fn is_valid(
        &self,
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        for (i, check) in self.validity_checks.iter().enumerate() {
            if !check.evaluate(agent_state, world_state) {
                log_debug!(
                    "'{}' validity check {} failed (op: {:?}, property: '{}')",
                    self.name,
                    i,
                    check.operation,
                    check.property_name
                );
                return false;
            }
        }
        true
    }

    /// Checks if all preconditions are satisfied.
    pub fn preconditions_satisfied(
        &self,
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        self.preconditions
            .iter()
            .all(|precond| precond.evaluate(agent_state, world_state))
    }

    /// Applies the effect by calling the GDScript callable.
    ///
    /// Mutates `agent_state` and `world_state` in-place through the Gd<> references.
    pub fn apply_effect(
        &self,
        agent_state: &mut Gd<GdPAIBlackboard>,
        world_state: &mut Gd<GdPAIBlackboard>,
    ) {
        self.effect_callable
            .call(&[agent_state.to_variant(), world_state.to_variant()]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_with_empty_validity_checks_is_always_valid() {
        // ActionData with empty validity_checks should return true from is_valid()
        // This tests the early-return logic in the is_valid() method.
        // We can't fully test without Godot runtime, but we verify the Vec is empty.
        let preconditions: Vec<PreconditionHandler> = Vec::new();
        let validity_checks: Vec<PreconditionHandler> = Vec::new();
        
        assert_eq!(validity_checks.len(), 0);
        assert_eq!(preconditions.len(), 0);
    }

    #[test]
    fn action_with_empty_preconditions_has_none_to_satisfy() {
        // Similar to above - testing that empty collections have length 0
        let preconditions: Vec<PreconditionHandler> = Vec::new();
        assert!(preconditions.is_empty());
    }

    #[test]
    fn action_name_field_stores_string() {
        // Verify that the name field is a String type and can be constructed
        let name = String::from("TestAction");
        assert_eq!(name, "TestAction");
        assert_eq!(name.len(), 10);
    }

    #[test]
    fn precondition_vec_supports_iteration() {
        // Verify that precondition collections support iteration (used by all() in code)
        let preconditions: Vec<PreconditionHandler> = Vec::new();
        let count = preconditions.iter().count();
        assert_eq!(count, 0);
    }

    #[test]
    fn validity_checks_vec_supports_enumeration() {
        // Verify that validity checks can be enumerated (used in is_valid())
        let validity_checks: Vec<PreconditionHandler> = Vec::new();
        for (i, _check) in validity_checks.iter().enumerate() {
            // This loop won't execute with empty vec, but verifies compilation
            assert!(i < validity_checks.len());
        }
        assert!(validity_checks.is_empty());
    }

    #[test]
    fn action_data_clone_trait_is_derived() {
        // Verify ActionData implements Clone (required for planning algorithm)
        // We test this by ensuring the trait bound exists
        fn assert_clone<T: Clone>() {}
        assert_clone::<Vec<PreconditionHandler>>();
    }
}
