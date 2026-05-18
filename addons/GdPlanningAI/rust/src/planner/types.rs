use crate::plan_types::*;
use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
use crate::requirement::{ProvisionSpec, RequirementSpec};
use std::sync::mpsc::Sender;

#[derive(Clone, Debug)]
pub struct PlanBranch {
    pub goal_preconditions: Vec<PreconditionSpec>,
    /// Physical needs of the FIRST action that are not met by InitialState.
    pub open_preconditions: Vec<PreconditionSpec>,
    /// Symbolic needs (Requirements) from anywhere in the chain not yet satisfied.
    /// Stores (ChainPosition, RequirementSpec)
    pub open_requirements: Vec<(usize, RequirementSpec)>,
    /// List of actions in execution order.
    pub action_chain: Vec<i64>,
    /// List of provisions provided by actions in the current chain.
    pub bound_provisions: Vec<ProvisionSpec>,
    /// List of (ActionIndex, ProvisionName, BoundValueList)
    pub action_bindings: Vec<(i64, String, Vec<VariantSnapshot>)>,
    /// Total cumulative cost from the latest forward simulation.
    pub cost: f64,
    /// The state after the last action in the chain.
    pub final_state_agent: BlackboardSnapshot,
    pub final_state_world: BlackboardSnapshot,
}

#[derive(Clone, Debug)]
pub struct ActionCandidate {
    pub action_idx: usize,
    pub estimated_cost: f64,
    pub satisfied_precondition_indices: Vec<usize>,
    pub satisfied_requirement_indices: Vec<usize>,
}

impl PlanBranch {
    pub fn new(
        goal_preconditions: &[PreconditionSpec],
        initial_provisions: &[ProvisionSpec],
        initial_agent: &BlackboardSnapshot,
        initial_world: &BlackboardSnapshot,
        request_tx: &Sender<CallbackRequest>,
    ) -> Self {
        // Any goal preconditions that are NOT met by InitialState are open.
        let mut open_preconditions = Vec::new();
        for pre in goal_preconditions {
            if !crate::planner::simulation::eval_precondition(
                pre,
                initial_agent,
                initial_world,
                initial_provisions.to_vec(),
                vec![], // No bindings for initial state
                request_tx,
            ) {
                open_preconditions.push(pre.clone());
            }
        }

        // Also extract requirements from initial state? 
        // Goal doesn't have symbolic requirements, only physical preconditions.

        Self {
            goal_preconditions: goal_preconditions.to_vec(),
            open_preconditions,
            open_requirements: vec![],
            action_chain: vec![],
            bound_provisions: initial_provisions.to_vec(), // Start with world provisions
            action_bindings: vec![],
            cost: 0.0,
            final_state_agent: initial_agent.clone(),
            final_state_world: initial_world.clone(),
        }
    }

    pub fn is_complete(
        &self,
        _initial_agent: &BlackboardSnapshot,
        _initial_world: &BlackboardSnapshot,
        request_tx: &Sender<CallbackRequest>,
    ) -> bool {
        // 1. Symbolic needs must be cleared
        if !self.open_requirements.is_empty() {
            return false;
        }

        // 2. Physical frontier must be grounded in the REAL InitialState
        if !self.open_preconditions.is_empty() {
            return false;
        }

        // 3. Deep Goal Check: The final state must actually satisfy the goal
        for goal_pre in &self.goal_preconditions {
            if !crate::planner::simulation::eval_precondition(
                goal_pre,
                &self.final_state_agent,
                &self.final_state_world,
                self.bound_provisions.clone(),
                vec![], // Deep goal check uses finalized state, but not specific action bindings
                request_tx,
            ) {
                return false;
            }
        }

        true
    }
}
