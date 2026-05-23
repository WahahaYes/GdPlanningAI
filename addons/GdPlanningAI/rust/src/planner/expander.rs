use crate::plan_types::*;
use crate::planner::types::{SearchContext, PlanBranch, DiscoveryResult};
use crate::planner::simulation::{StepResult, simulate_action, eval_precondition};
use crate::requirement::{provision_satisfies_requirement, RequirementSpec, ProvisionSpec};


pub struct Candidate {
    pub action_idx: usize,
    pub satisfied_requirements: Vec<(RequirementSpec, ProvisionSpec)>,
}

pub fn find_candidates(
    branch: &PlanBranch,
    ctx: &SearchContext,
    response: Option<&CallbackResponse>,
) -> StepResult<Vec<Candidate>> {
    let mut candidates = Vec::new();
    let mut some_pending = false;
    let mut last_pending_id = 0;

    for (idx, action) in ctx.actions.iter().enumerate() {
        // 0. Validity filter (against InitialState)
        let mut validity_failed = false;
        for check in &action.validity_checks {
            match eval_precondition(check, &ctx.initial_agent, &ctx.initial_world, ctx, response) {
                StepResult::Ready(true) => {}
                StepResult::Ready(false) => {
                    validity_failed = true;
                    break;
                }
                StepResult::Pending(id) => {
                    some_pending = true;
                    last_pending_id = id;
                    validity_failed = true; // Skip this action for now, but we are pending
                    break;
                }
                StepResult::Invalid => {
                    validity_failed = true;
                    break;
                }
            }
        }
        if validity_failed { continue; }

        // 1. Symbolic match (Provisions satisfy Requirements)
        let mut satisfied_requirements = Vec::new();
        for (_, req) in &branch.open_requirements {
            for prov in &action.provisions {
                if provision_satisfies_requirement(prov, req, None) {
                    satisfied_requirements.push((req.clone(), prov.clone()));
                }
            }
        }

        let satisfies_requirement = !satisfied_requirements.is_empty();

        // 2. Simulation match (Effect satisfies Preconditions)
        let mut satisfies_precondition = false;
        if !branch.open_preconditions.is_empty() {
            let mut res_to_check = None;
            
            // Check cache
            {
                let cache = ctx.discovery_results.lock().unwrap();
                if let Some(res) = cache.get(&idx) {
                    res_to_check = Some(res.clone());
                }
            }

            if res_to_check.is_none() {
                // Not in cache, check if pending
                let is_pending = {
                    let pending = ctx.discovery_pending.lock().unwrap();
                    pending.contains_key(&idx)
                };

                if is_pending {
                    some_pending = true;
                    // We don't have the result yet, but we've already requested it.
                } else {
                    // Start discovery simulation against InitialState
                    let mut dummy_costs = vec![-1.0];
                    match simulate_action(idx, &ctx.initial_agent, &ctx.initial_world, ctx, response, &mut dummy_costs, 0) {
                        StepResult::Ready(res) => {
                            let disc_res = DiscoveryResult {
                                agent: res.agent,
                                world: res.world,
                                cost: res.cost,
                            };
                            let mut cache = ctx.discovery_results.lock().unwrap();
                            cache.insert(idx, disc_res.clone());
                            res_to_check = Some(disc_res);
                        }
                        StepResult::Pending(id) => {
                            let mut pending = ctx.discovery_pending.lock().unwrap();
                            pending.insert(idx, id);
                            let mut req_map = ctx.discovery_request_map.lock().unwrap();
                            req_map.insert(id, idx);
                            some_pending = true;
                            last_pending_id = id;
                        }
                        StepResult::Invalid => {}
                    }
                }
            }

            if let Some(res) = res_to_check {
                for (_, pre) in &branch.open_preconditions {
                    if let Some(true) = pre.evaluate_builtin(&res.agent, &res.world) {
                        satisfies_precondition = true;
                        break;
                    }
                }
            }
        }

        if satisfies_requirement || satisfies_precondition {
            candidates.push(Candidate {
                action_idx: idx,
                satisfied_requirements,
            });
        }
    }

    if some_pending && candidates.is_empty() {
        // If we didn't find any ready candidates but some actions are still being discovered,
        // yield to wait for discovery.
        StepResult::Pending(last_pending_id)
    } else {
        // If we found candidates, return them even if some other actions are still pending discovery.
        // Or if nothing is pending and no candidates found, return empty list.
        StepResult::Ready(candidates)
    }
}
