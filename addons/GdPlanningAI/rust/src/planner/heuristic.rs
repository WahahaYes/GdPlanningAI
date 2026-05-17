use super::expander::PlanBranch;

pub fn estimate_remaining(branch: &PlanBranch, _min_action_cost: f64, _min_provision_cost: f64) -> f64 {
    let precondition_estimate = branch.open_preconditions.len() as f64 * _min_action_cost;
    let requirement_estimate = branch.open_requirements.len() as f64 * _min_provision_cost;
    precondition_estimate + requirement_estimate
}
