//! Candidate action discovery for the planning engine.
//!
//! This module provides functions to identify which actions can satisfy the current
//! open needs (preconditions and requirements) of a plan branch.

use crate::plan_types::*;
use crate::planner::simulation::{SimArgs, StepResult, eval_precondition, simulate_action};
use crate::planner::types::{
    BindingMap, DiscoveryRequest, DiscoveryResult, PlanBranch, SearchContext,
};
use crate::requirement::{ProvisionSpec, RequirementSpec, provision_satisfies_requirement};
use std::collections::HashSet;

/// A candidate action that can potentially satisfy one or more open needs.
pub struct Candidate {
    pub action_idx: usize,
    pub satisfied_requirements: Vec<(usize, RequirementSpec, ProvisionSpec)>,
    pub satisfied_preconditions: Vec<usize>, // Indices into branch.open_preconditions
    pub bindings: BindingMap,
}

fn get_bindings_for_match(prov: &ProvisionSpec, req: &RequirementSpec) -> BindingMap {
    let mut bindings = Vec::new();
    match (prov, req) {
        (
            ProvisionSpec::Binding {
                binding_name,
                value,
            },
            _,
        ) => {
            bindings.push((binding_name.clone(), vec![value.clone()]));
        }
        (ProvisionSpec::Fact { fact_name, args }, _) => {
            bindings.push((fact_name.clone(), args.clone()));
        }
        (ProvisionSpec::FactWildcard { fact_name }, RequirementSpec::Fact { args, .. }) => {
            bindings.push((fact_name.clone(), args.clone()));
        }
        _ => {}
    }
    bindings
}

fn get_discovery_result(
    action_idx: usize,
    bindings: &BindingMap,
    ctx: &SearchContext,
    response: Option<&CallbackResponse>,
) -> StepResult<DiscoveryResult> {
    // 1. Check cache
    {
        let cache = ctx.discovery_results.lock().unwrap();
        if let Some(res) = cache.get(&(action_idx, bindings.clone())) {
            return StepResult::Ready(res.clone());
        }
    }

    // 2. Check if pending
    {
        let pending = ctx.discovery_pending.lock().unwrap();
        if let Some(&id) = pending.get(&(action_idx, bindings.clone())) {
            return StepResult::Pending(id);
        }
    }

    // 3. Start discovery simulation
    let mut cost_cache = {
        let costs = ctx.discovery_costs.lock().unwrap();
        vec![
            costs
                .get(&(action_idx, bindings.clone()))
                .cloned()
                .unwrap_or(-1.0),
        ]
    };

    match simulate_action(
        action_idx,
        SimArgs {
            agent: &ctx.initial_agent,
            world: &ctx.initial_world,
            ctx,
            response,
            branch_action_costs: &mut cost_cache,
            simulation_index: 0,
            bindings,
        },
    ) {
        StepResult::Ready(res) => {
            let disc_res = DiscoveryResult {
                agent: res.agent,
                world: res.world,
                cost: res.cost,
            };
            let mut cache = ctx.discovery_results.lock().unwrap();
            cache.insert((action_idx, bindings.clone()), disc_res.clone());
            StepResult::Ready(disc_res)
        }
        StepResult::Pending(id) => {
            if cost_cache[0] >= 0.0 {
                let mut costs = ctx.discovery_costs.lock().unwrap();
                costs.insert((action_idx, bindings.clone()), cost_cache[0]);
            }
            let mut pending = ctx.discovery_pending.lock().unwrap();
            pending.insert((action_idx, bindings.clone()), id);
            let mut req_map = ctx.discovery_request_map.lock().unwrap();
            req_map.insert(
                id,
                DiscoveryRequest::Simulation(action_idx, bindings.clone()),
            );
            StepResult::Pending(id)
        }
        StepResult::Invalid => StepResult::Invalid,
        StepResult::Complete => unreachable!(),
    }
}

/// Finds all candidate actions that satisfy at least one open need of the given branch.
///
/// This function performs a hybrid discovery process:
/// 1. Symbolic Layer: Matches action Provisions against open Requirements.
/// 2. Simulation Layer: Matches action effects (via Discovery simulation) against open Preconditions.
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
                // Custom check, check discovery cache (using empty bindings for validity)
                let empty_bindings = Vec::new();
                let cache = ctx.discovery_precond_results.lock().unwrap();
                if let Some(&res) = cache.get(&(idx, check.clone(), empty_bindings.clone())) {
                    if !res {
                        validity_failed = true;
                        break;
                    }
                } else {
                    // Not in cache, check if pending
                    let mut pending = ctx.discovery_precond_pending.lock().unwrap();
                    if let Some(&id) = pending.get(&(idx, check.clone(), empty_bindings.clone())) {
                        some_pending = true;
                        last_pending_id = id;
                        validity_failed = true;
                        break;
                    } else {
                        // Start request
                        match eval_precondition(
                            check,
                            &ctx.initial_agent,
                            &ctx.initial_world,
                            ctx,
                            response,
                            &empty_bindings,
                        ) {
                            StepResult::Ready(res) => {
                                if !res {
                                    validity_failed = true;
                                    break;
                                }
                            }
                            StepResult::Pending(id) => {
                                pending.insert((idx, check.clone(), empty_bindings.clone()), id);
                                let mut req_map = ctx.discovery_request_map.lock().unwrap();
                                req_map.insert(
                                    id,
                                    DiscoveryRequest::Precondition(
                                        idx,
                                        check.clone(),
                                        empty_bindings,
                                    ),
                                );
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
        if validity_failed {
            continue;
        }

        let has_wildcard = action
            .provisions
            .iter()
            .any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }));

        if has_wildcard {
            // Per-requirement candidates for actions with wildcards
            for (req_idx, (_pos, req)) in branch.open_requirements.iter().enumerate() {
                for prov in &action.provisions {
                    if provision_satisfies_requirement(prov, req, Some(&ctx.initial_world)) {
                        let bindings = get_bindings_for_match(prov, req);
                        match get_discovery_result(idx, &bindings, ctx, response) {
                            StepResult::Ready(res) => {
                                let mut satisfied_preconditions = Vec::new();
                                for (pre_idx, (pos, pre)) in
                                    branch.open_preconditions.iter().enumerate()
                                {
                                    if let Some(eval_res) =
                                        pre.evaluate_builtin(&res.agent, &res.world)
                                    {
                                        if eval_res {
                                            satisfied_preconditions.push(pre_idx);
                                        }
                                    } else {
                                        // Custom precond check with specific discovery bindings
                                        let cache = ctx.discovery_precond_results.lock().unwrap();
                                        if let Some(&eval_res) =
                                            cache.get(&(idx, pre.clone(), bindings.clone()))
                                        {
                                            if eval_res {
                                                satisfied_preconditions.push(pre_idx);
                                            }
                                        } else {
                                            let mut pending =
                                                ctx.discovery_precond_pending.lock().unwrap();
                                            if let Some(&id) =
                                                pending.get(&(idx, pre.clone(), bindings.clone()))
                                            {
                                                some_pending = true;
                                                last_pending_id = id;
                                            } else {
                                                match eval_precondition(
                                                    pre, &res.agent, &res.world, ctx, response,
                                                    &bindings,
                                                ) {
                                                    StepResult::Ready(eval_res) => {
                                                        if eval_res {
                                                            satisfied_preconditions.push(pre_idx);
                                                        }
                                                    }
                                                    StepResult::Pending(id) => {
                                                        pending.insert(
                                                            (idx, pre.clone(), bindings.clone()),
                                                            id,
                                                        );
                                                        let mut req_map = ctx
                                                            .discovery_request_map
                                                            .lock()
                                                            .unwrap();
                                                        req_map.insert(
                                                            id,
                                                            DiscoveryRequest::Precondition(
                                                                idx,
                                                                pre.clone(),
                                                                bindings.clone(),
                                                            ),
                                                        );
                                                        some_pending = true;
                                                        last_pending_id = id;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                                candidates.push(Candidate {
                                    action_idx: idx,
                                    satisfied_requirements: vec![(
                                        req_idx,
                                        req.clone(),
                                        prov.clone(),
                                    )],
                                    satisfied_preconditions,
                                    bindings,
                                });
                            }
                            StepResult::Pending(id) => {
                                some_pending = true;
                                last_pending_id = id;
                            }
                            _ => {}
                        }
                    }
                }
            }
        } else {
            // Grouped candidates for actions without wildcards
            let mut satisfied_requirements = Vec::new();
            let mut matched_req_indices = HashSet::new();
            for (req_idx, (_pos, req)) in branch.open_requirements.iter().enumerate() {
                for prov in &action.provisions {
                    if provision_satisfies_requirement(prov, req, Some(&ctx.initial_world)) {
                        if matched_req_indices.insert(req_idx) {
                            satisfied_requirements.push((req_idx, req.clone(), prov.clone()));
                        }
                        break; // One provision is enough for this requirement
                    }
                }
            }

            let empty_bindings = Vec::new();
            match get_discovery_result(idx, &empty_bindings, ctx, response) {
                StepResult::Ready(res) => {
                    let mut satisfied_preconditions = Vec::new();
                    for (pre_idx, (pos, pre)) in branch.open_preconditions.iter().enumerate() {
                        if let Some(eval_res) = pre.evaluate_builtin(&res.agent, &res.world) {
                            if eval_res {
                                satisfied_preconditions.push(pre_idx);
                            }
                        } else {
                            let cache = ctx.discovery_precond_results.lock().unwrap();
                            if let Some(&eval_res) =
                                cache.get(&(idx, pre.clone(), empty_bindings.clone()))
                            {
                                if eval_res {
                                    satisfied_preconditions.push(pre_idx);
                                }
                            } else {
                                let mut pending = ctx.discovery_precond_pending.lock().unwrap();
                                if let Some(&id) =
                                    pending.get(&(idx, pre.clone(), empty_bindings.clone()))
                                {
                                    some_pending = true;
                                    last_pending_id = id;
                                } else {
                                    match eval_precondition(
                                        pre,
                                        &res.agent,
                                        &res.world,
                                        ctx,
                                        response,
                                        &empty_bindings,
                                    ) {
                                        StepResult::Ready(eval_res) => {
                                            if eval_res {
                                                satisfied_preconditions.push(pre_idx);
                                            }
                                        }
                                        StepResult::Pending(id) => {
                                            pending.insert(
                                                (idx, pre.clone(), empty_bindings.clone()),
                                                id,
                                            );
                                            let mut req_map =
                                                ctx.discovery_request_map.lock().unwrap();
                                            req_map.insert(
                                                id,
                                                DiscoveryRequest::Precondition(
                                                    idx,
                                                    pre.clone(),
                                                    empty_bindings.clone(),
                                                ),
                                            );
                                            some_pending = true;
                                            last_pending_id = id;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    if !satisfied_requirements.is_empty() || !satisfied_preconditions.is_empty() {
                        candidates.push(Candidate {
                            action_idx: idx,
                            satisfied_requirements,
                            satisfied_preconditions,
                            bindings: empty_bindings,
                        });
                    }
                }
                StepResult::Pending(id) => {
                    some_pending = true;
                    last_pending_id = id;
                }
                _ => {}
            }
        }
    }

    if some_pending {
        StepResult::Pending(last_pending_id)
    } else {
        StepResult::Ready(candidates)
    }
}
