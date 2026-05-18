use super::types::PlanBranch;

pub fn estimate_remaining(branch: &PlanBranch, min_action_cost: f64) -> f64 {
    // Admissible heuristic: count open needs
    let pre_count = branch.open_preconditions.len() as f64;
    let req_count = branch.open_requirements.len() as f64;
    
    (pre_count + req_count) * min_action_cost
}
