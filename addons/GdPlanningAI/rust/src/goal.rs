//! Goal data holder.
//!
//! A [`GoalData`] captures the reward, UID, and desired state of a GDScript
//! `Goal` object. Goals are sorted by descending reward before the planning
//! search begins.

use super::gdpai_blackboard::GdPAIBlackboard;
use super::precondition::PreconditionHandler;
use godot::prelude::*;

/// Deserialized representation of a GDScript `Goal` used during planning.
///
/// Holds the goal's unique ID, reward value, and the set of preconditions
/// that must all be satisfied for the goal to be considered achieved.
#[derive(Clone, Debug)]
pub struct GoalData {
    pub name: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionHandler>,
    pub original_index: usize,
}

impl GoalData {
    /// Deserializes a [`GoalData`] from a Godot dictionary.
    ///
    /// Required keys: `name` (String), `reward` (float).
    /// Optional key: `desired_state` (Array\[Dictionary\]) — defaults to empty.
    /// Returns `None` if any required key is missing or has an incompatible type.
    pub fn from_dict(dict: &VarDictionary) -> Option<Self> {
        let name = match dict.get("name").and_then(|v| v.try_to::<String>().ok()) {
            Some(n) => n,
            None => {
                log_warn!("GoalData: dict is missing a valid 'name'");
                return None;
            }
        };
        let reward = match dict.get("reward").and_then(|v| v.try_to::<f64>().ok()) {
            Some(r) => r,
            None => {
                log_warn!("GoalData '{}': dict is missing a valid 'reward'", name);
                return None;
            }
        };

        let desired_state = dict
            .get("desired_state")
            .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
            .map(|arr| {
                arr.iter_shared()
                    .filter_map(|d| PreconditionHandler::from_dict(&d))
                    .collect()
            })
            .unwrap_or_default();

        Some(Self {
            name,
            reward,
            desired_state,
            original_index: 0,
        })
    }

    /// Checks if the goal is satisfied in the given state.
    pub fn is_satisfied(
        &self,
        agent_state: &Gd<GdPAIBlackboard>,
        world_state: &Gd<GdPAIBlackboard>,
    ) -> bool {
        self.desired_state
            .iter()
            .all(|precond| precond.evaluate(agent_state, world_state))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_goal(name: &str, reward: f64, original_index: usize) -> GoalData {
        GoalData {
            name: name.to_string(),
            reward,
            desired_state: vec![],
            original_index,
        }
    }

    #[test]
    fn goals_sort_descending_by_reward() {
        let mut goals = vec![
            make_goal("Low Value", 10.0, 0),
            make_goal("High Value", 100.0, 1),
            make_goal("Mid Value", 50.0, 2),
        ];
        goals.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(goals[0].name, "High Value");
        assert_eq!(goals[1].name, "Mid Value");
        assert_eq!(goals[2].name, "Low Value");
    }

    #[test]
    fn original_index_preserved_after_sort() {
        let mut goals = vec![
            make_goal("First", 30.0, 0),
            make_goal("Second", 80.0, 1),
            make_goal("Third", 10.0, 2),
        ];
        goals.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap_or(std::cmp::Ordering::Equal));
        assert_eq!(goals[0].original_index, 1); // "Second" was at GDScript index 1
        assert_eq!(goals[1].original_index, 0); // "First" was at GDScript index 0
        assert_eq!(goals[2].original_index, 2); // "Third" was at GDScript index 2
    }
}
