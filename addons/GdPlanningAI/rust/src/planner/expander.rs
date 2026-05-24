use crate::plan_types::*;
use crate::planner::types::{SearchContext, PlanBranch, DiscoveryResult, DiscoveryRequest};
use crate::planner::simulation::{StepResult, simulate_action, eval_precondition};
use crate::requirement::{provision_satisfies_requirement, RequirementSpec, ProvisionSpec};


pub struct Candidate {
    pub action_idx: usize,
    pub satisfied_requirements: Vec<(usize, RequirementSpec, ProvisionSpec)>,
    pub satisfied_preconditions: Vec<usize>, // Indices into branch.open_preconditions
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
            if let Some(res) = check.evaluate_builtin(&ctx.initial_agent, &ctx.initial_world) {
                if !res {
                    validity_failed = true;
                    break;
                }
            } else {
                // Custom check, check discovery cache
                let cache = ctx.discovery_precond_results.lock().unwrap();
                if let Some(&res) = cache.get(&(idx, check.clone())) {
                    if !res {
                        validity_failed = true;
                        break;
                    }
                } else {
                    // Not in cache, check if pending
                    let mut pending = ctx.discovery_precond_pending.lock().unwrap();
                    if let Some(&id) = pending.get(&(idx, check.clone())) {
                        some_pending = true;
                        last_pending_id = id;
                        validity_failed = true;
                        break;
                    } else {
                        // Start request
                        match eval_precondition(check, &ctx.initial_agent, &ctx.initial_world, ctx, response) {
                            StepResult::Ready(res) => {
                                // This can happen if the response is actually for this check
                                if !res {
                                    validity_failed = true;
                                    break;
                                }
                            }
                            StepResult::Pending(id) => {
                                pending.insert((idx, check.clone()), id);
                                let mut req_map = ctx.discovery_request_map.lock().unwrap();
                                req_map.insert(id, DiscoveryRequest::Precondition(idx, check.clone()));
                                some_pending = true;
                                last_pending_id = id;
                                validity_failed = true;
                                break;
                            }
                            _ => {
                                validity_failed = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        if validity_failed { continue; }

        // 1. Symbolic match (Provisions satisfy Requirements)
        let mut satisfied_requirements = Vec::new();
        for (req_idx, (pos, req)) in branch.open_requirements.iter().enumerate() {
            for prov in &action.provisions {
                if provision_satisfies_requirement(prov, req, None) {
                    satisfied_requirements.push((req_idx, req.clone(), prov.clone()));
                }
            }
        }

        let satisfies_requirement = !satisfied_requirements.is_empty();

        // 2. Simulation match (Effect satisfies Preconditions)
        let mut satisfied_preconditions = Vec::new();
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
                let mut pending = ctx.discovery_pending.lock().unwrap();
                if let Some(&id) = pending.get(&idx) {
                    some_pending = true;
                    last_pending_id = id;
                } else {
                    // Start discovery simulation against InitialState
                    let mut cost_cache = {
                        let costs = ctx.discovery_costs.lock().unwrap();
                        vec![costs.get(&idx).cloned().unwrap_or(-1.0)]
                    };

                    match simulate_action(idx, &ctx.initial_agent, &ctx.initial_world, ctx, response, &mut cost_cache, 0) {
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
                            // Update cost cache if we got it in this step
                            if cost_cache[0] >= 0.0 {
                                let mut costs = ctx.discovery_costs.lock().unwrap();
                                costs.insert(idx, cost_cache[0]);
                            }

                            pending.insert(idx, id);
                            let mut req_map = ctx.discovery_request_map.lock().unwrap();
                            req_map.insert(id, DiscoveryRequest::Simulation(idx));
                            some_pending = true;
                            last_pending_id = id;
                        }
                        StepResult::Invalid => {}
                        StepResult::Complete => unreachable!("simulate_action cannot return Complete during discovery"),
                    }
                }
            }

            if let Some(res) = res_to_check {
                for (pre_idx, (_pos, pre)) in branch.open_preconditions.iter().enumerate() {
                    if let Some(eval_res) = pre.evaluate_builtin(&res.agent, &res.world) {
                        if eval_res {
                            satisfied_preconditions.push(pre_idx);
                            satisfies_precondition = true;
                        }
                    } else {
                        // Custom check, check discovery cache
                        let cache = ctx.discovery_precond_results.lock().unwrap();
                        if let Some(&eval_res) = cache.get(&(idx, pre.clone())) {
                            if eval_res {
                                satisfied_preconditions.push(pre_idx);
                                satisfies_precondition = true;
                            }
                        } else {
                            // Not in cache, check if pending
                            let mut pending = ctx.discovery_precond_pending.lock().unwrap();
                            if let Some(&id) = pending.get(&(idx, pre.clone())) {
                                some_pending = true;
                                last_pending_id = id;
                            } else {
                                // Start request
                                match eval_precondition(pre, &res.agent, &res.world, ctx, response) {
                                    StepResult::Ready(eval_res) => {
                                        if eval_res {
                                            satisfied_preconditions.push(pre_idx);
                                            satisfies_precondition = true;
                                        }
                                    }
                                    StepResult::Pending(id) => {
                                        pending.insert((idx, pre.clone()), id);
                                        let mut req_map = ctx.discovery_request_map.lock().unwrap();
                                        req_map.insert(id, DiscoveryRequest::Precondition(idx, pre.clone()));
                                        some_pending = true;
                                        last_pending_id = id;
                                    }
                                    _ => {}
                                }
                            }
                        }
                    }
                }
            }
        }

        if satisfies_requirement || satisfies_precondition {
            candidates.push(Candidate {
                action_idx: idx,
                satisfied_requirements,
                satisfied_preconditions,
            });
        }
    }

    if some_pending {
        // If any actions are still being discovered, yield to wait for discovery.
        // We MUST NOT return Ready yet, otherwise the node will be marked as 'visited'
        // and we will miss the candidates that are currently pending.
        StepResult::Pending(last_pending_id)
    } else {
        // All actions have been fully evaluated (either cached, ready, or invalid).
        StepResult::Ready(candidates)
    }
}
