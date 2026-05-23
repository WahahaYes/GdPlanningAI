//! Plan result type shared between planner and scheduler.

use crate::plan_types::PreconditionSpec;
use crate::requirement::RequirementSpec;

/// Result of the planning algorithm.
#[derive(Clone, Debug)]
pub struct PlanResult {
    pub success: bool,
    pub action_chain: Vec<i64>,
    pub total_cost: f64,
    /// Index into the original goals array passed from GDScript; -1 on failure
    pub goal_index: i64,
    /// Indices of actions that used placeholder simulation (requirements unresolved at eval time).
    /// These may need re-simulation with actual cost/effects for accurate planning.
    pub deferred_action_indices: Vec<i64>,
    /// Action-specific bindings: chain_position -> (fact_name, values)
    /// Each action occurrence gets its own bound values from wildcard provisions during planning.
    pub action_bindings: Vec<(i64, String, Vec<crate::snapshot::VariantSnapshot>)>,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct PlanFingerprint {
    pub open_preconditions: Vec<PreconditionSpec>,
    pub open_requirements: Vec<(usize, RequirementSpec)>,
}

impl PlanResult {
    /// Returns a [`PlanResult`] representing a failed plan.
    pub fn failure() -> Self {
        Self {
            success: false,
            action_chain: vec![],
            total_cost: f64::INFINITY,
            goal_index: -1,
            deferred_action_indices: vec![],
            action_bindings: vec![],
        }
    }
}
