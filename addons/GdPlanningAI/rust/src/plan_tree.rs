//! The plan result type shared between the planner and the scheduler.
//!
//! [`PlanResult`] is produced on a planner thread by
//! [`crate::planner::PlannerEngine`] and converted to a GDScript dictionary by the scheduler
//! on the main thread before it reaches the agent's `_on_plan_ready`.

/// Result of the planning algorithm.
#[derive(Clone, Debug)]
pub struct PlanResult {
    /// True when a complete, verified plan was found (an empty chain is a valid plan
    /// when the goal already holds).
    pub success: bool,
    /// Indices into the actions array passed to `submit_plan`, in execution order.
    pub action_chain: Vec<i64>,
    /// Sum of the action costs of the chain. Not meaningful on failure: [`PlanResult::failure`]
    /// reports infinity while failures built inside the engine and scheduler report 0.
    pub total_cost: f64,
    /// Index into the original goals array passed from GDScript; -1 on failure
    pub goal_index: i64,
    /// Reserved; the engine never populates it.
    pub deferred_action_indices: Vec<i64>,
    /// `(chain_position, fact_name, values)` bound for each action occurrence; a position can
    /// appear once per binding name.
    pub action_bindings: Vec<(i64, String, Vec<crate::snapshot::VariantSnapshot>)>,
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
