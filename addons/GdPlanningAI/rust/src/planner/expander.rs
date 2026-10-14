//! Candidate action discovery for the planning engine.
//!
//! This module provides functions to identify which actions can satisfy the current
//! open needs (preconditions and requirements) of a plan branch.
//!
//! Discovery is a hybrid of two matching layers. The symbolic layer matches an action's
//! provisions against open requirements. The simulation layer simulates the action once against
//! the *initial* state (cached per `(action, bindings)` in the shared [`SearchContext`]) and
//! checks which open preconditions its resulting state satisfies. Because the simulation may
//! need GDScript, discovery is resumable: an unanswered callback makes [`find_candidates`]
//! report a pending request id, the engine parks the node, and discovery is simply run again
//! once the answer is cached.

use crate::planner::simulation::{SimArgs, StepResult, eval_precondition, simulate_action};
use crate::planner::types::{
    BindingMap, DiscoveryRequest, DiscoveryResult, PlanBranch, ProvisionKind, SearchContext,
    binding_values_from_match,
};
use crate::requirement::{ProvisionSpec, RequirementSpec, provision_satisfies_requirement};
use std::collections::{BTreeSet, HashMap};

/// A candidate action that can potentially satisfy one or more open needs.
pub struct Candidate {
    /// Index into [`SearchContext::actions`].
    pub action_idx: usize,
    /// Open requirements this action satisfies, as `(index into branch.open_requirements,
    /// the requirement, the provision that matched it)`.
    pub satisfied_requirements: Vec<(usize, RequirementSpec, ProvisionSpec)>,
    /// Indices into `branch.open_preconditions` that hold in the action's simulated post-state.
    pub satisfied_preconditions: Vec<usize>,
    /// Concrete bindings this candidate was discovered with: empty for ordinary actions, the
    /// matched requirement's arguments for wildcard actions.
    pub bindings: BindingMap,
}

/// Bindings a provision/requirement match contributes: the single wildcard or binding entry,
/// or nothing. See [`binding_values_from_match`].
fn get_bindings_for_match(prov: &ProvisionSpec, req: &RequirementSpec) -> BindingMap {
    let (name, values) = binding_values_from_match(prov, req);
    if name.is_empty() {
        Vec::new()
    } else {
        vec![(name, values)]
    }
}

/// Checks whether `key` is present in `pending_map`.
///
/// If the associated request ID still exists in `request_map`, the entry is
/// genuinely pending and the ID is returned. Otherwise the entry is stale
/// (its response was already processed but the pending map was not cleared)
/// and it is removed, returning `None`.
///
/// Locks are taken one at a time and released before relocking for removal, so no lock is held
/// across the other and callbacks are never fired while holding one.
fn check_pending_or_clean_stale<K>(
    pending_map: &std::sync::Mutex<HashMap<K, usize>>,
    request_map: &std::sync::Mutex<HashMap<usize, DiscoveryRequest>>,
    key: &K,
) -> Option<usize>
where
    K: Clone + Eq + std::hash::Hash,
{
    let pending = pending_map.lock().unwrap();
    if let Some(&id) = pending.get(key) {
        let req_map = request_map.lock().unwrap();
        if req_map.contains_key(&id) {
            return Some(id);
        } else {
            drop(req_map);
            drop(pending);
            let mut pending = pending_map.lock().unwrap();
            pending.remove(key);
        }
    }
    None
}

/// Returns the simulated outcome of running `action_idx` once against the initial state with
/// `bindings`, starting the simulation if it is not cached.
///
/// Returns `Ready` from the cache (or when the simulation needs no callback), `Pending(id)`
/// when a callback for this exact `(action, bindings)` is already in flight or was just
/// fired, and `Invalid` when the action cannot run (for example its cost is infinite). A
/// pending request is registered in the context's pending and request maps so the engine can
/// route the answer into the right cache.
fn lookup_or_start_discovery(
    action_idx: usize,
    bindings: &BindingMap,
    ctx: &SearchContext,
) -> StepResult<DiscoveryResult> {
    // 1. Check cache
    {
        let cache = ctx.discovery_results.lock().unwrap();
        if let Some(res) = cache.get(&(action_idx, bindings.clone())) {
            return StepResult::Ready(res.clone());
        }
    }

    // 2. Check if pending (defend against stale entries)
    if let Some(id) = check_pending_or_clean_stale(
        &ctx.discovery_pending,
        &ctx.discovery_request_map,
        &(action_idx, bindings.clone()),
    ) {
        return StepResult::Pending(id);
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
            response: None,
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

/// Ready candidates and any pending callback ID from candidate discovery.
pub struct CandidatesResult {
    /// Candidates whose discovery needed no further callbacks; safe to expand now.
    pub ready: Vec<Candidate>,
    /// Id of one outstanding callback that blocked other candidates, if any (when several
    /// are outstanding, the last one seen). The engine parks the node under it and reruns
    /// discovery when that answer arrives.
    pub pending_id: Option<usize>,
}

/// Finds all candidate actions that satisfy at least one open need of the given branch.
///
/// This function performs a hybrid discovery process:
/// 1. Symbolic Layer: Matches action Provisions against open Requirements.
/// 2. Simulation Layer: Matches action effects (via Discovery simulation) against open Preconditions.
///
/// Uses the provision index to avoid scanning actions that cannot possibly satisfy any open need.
///
/// Every considered action is first filtered by its `validity_checks` against the initial
/// state; a custom check that is still in flight skips the action for now and is reported as
/// pending. The rest depends on whether the action has a wildcard provision:
/// - Wildcard actions (such as GoTo) are discovered once per matching (requirement, provision)
///   pair, with bindings taken from the requirement, so that a single generic action yields a
///   separate candidate for every concrete target that is needed.
/// - Other actions are discovered once with empty bindings and may satisfy several
///   requirements at once.
///
/// Open preconditions are satisfied when they hold in the action's simulated post-state; custom
/// ones are evaluated through callbacks and cached by `(action, precondition, bindings)`.
/// A candidate must satisfy at least one requirement or precondition to be emitted (the
/// wildcard path always has one by construction).
pub fn find_candidates(branch: &PlanBranch, ctx: &SearchContext) -> CandidatesResult {
    let mut candidates = Vec::new();
    let mut some_pending = false;
    let mut last_pending_id = 0;

    // 1. Determine which actions to evaluate
    let mut candidate_actions: BTreeSet<usize> = BTreeSet::new();

    // a) Actions whose provisions match any open requirement
    for (_pos, req) in branch.open_requirements.iter() {
        let lookup_key = match req {
            RequirementSpec::BindingExists { binding_name }
            | RequirementSpec::BindingEquals { binding_name, .. }
            | RequirementSpec::BindingInSet { binding_name, .. } => {
                (ProvisionKind::Binding, binding_name.clone())
            }
            RequirementSpec::Fact { fact_name, .. } => (ProvisionKind::Fact, fact_name.clone()),
        };
        if let Some(action_indices) = ctx.provision_index.get(&lookup_key) {
            candidate_actions.extend(action_indices);
        }
        // Fact requirements may also be satisfied by FactWildcard provisions
        if matches!(req, RequirementSpec::Fact { .. }) {
            let wildcard_key = (ProvisionKind::FactWildcard, lookup_key.1);
            if let Some(action_indices) = ctx.provision_index.get(&wildcard_key) {
                candidate_actions.extend(action_indices);
            }
        }
    }

    // b) Non-wildcard actions may satisfy preconditions through their effects.
    //    Always include them; the per-action loop will determine whether they
    //    actually satisfy any open need.
    candidate_actions.extend(&ctx.non_wildcard_actions);

    // 2. Evaluate each candidate action
    for idx in candidate_actions {
        let action = &ctx.actions[idx];

        // 0. Validity filter (against InitialState)
        let mut validity_failed = false;
        for check in &action.validity_checks {
            if let Some(res) = check.evaluate_builtin(&ctx.initial_agent, &ctx.initial_world) {
                if !res {
                    validity_failed = true;
                    break;
                }
            } else {
                // Custom check - use short-lived locks so no lock is held when firing callbacks.
                let empty_bindings: BindingMap = Vec::new();

                // 1. Check cache
                let cached = {
                    let cache = ctx.discovery_precond_results.lock().unwrap();
                    cache
                        .get(&(idx, check.clone(), empty_bindings.clone()))
                        .copied()
                };
                if let Some(res) = cached {
                    if !res {
                        validity_failed = true;
                        break;
                    }
                    continue; // cached true — move to next check
                }

                // 2. Check pending (and defend against stale entries)
                if let Some(id) = check_pending_or_clean_stale(
                    &ctx.discovery_precond_pending,
                    &ctx.discovery_request_map,
                    &(idx, check.clone(), empty_bindings.clone()),
                ) {
                    some_pending = true;
                    last_pending_id = id;
                    validity_failed = true;
                    break;
                }

                // 3. Fire callback (no locks held)
                match eval_precondition(
                    check,
                    &ctx.initial_agent,
                    &ctx.initial_world,
                    ctx,
                    None,
                    &empty_bindings,
                ) {
                    StepResult::Ready(res) => {
                        // Cache immediately so subsequent calls don't re-fire
                        let mut cache = ctx.discovery_precond_results.lock().unwrap();
                        cache.insert((idx, check.clone(), empty_bindings.clone()), res);
                        drop(cache);
                        if !res {
                            validity_failed = true;
                            break;
                        }
                    }
                    StepResult::Pending(id) => {
                        let mut pending = ctx.discovery_precond_pending.lock().unwrap();
                        pending.insert((idx, check.clone(), empty_bindings.clone()), id);
                        drop(pending);
                        let mut req_map = ctx.discovery_request_map.lock().unwrap();
                        req_map.insert(
                            id,
                            DiscoveryRequest::Precondition(idx, check.clone(), empty_bindings),
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
                        match lookup_or_start_discovery(idx, &bindings, ctx) {
                            StepResult::Ready(res) => {
                                let mut satisfied_preconditions = Vec::new();
                                let mut precond_pending_for_this_candidate = false;
                                for (pre_idx, (_pos, pre)) in
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
                                            if let Some(id) = check_pending_or_clean_stale(
                                                &ctx.discovery_precond_pending,
                                                &ctx.discovery_request_map,
                                                &(idx, pre.clone(), bindings.clone()),
                                            ) {
                                                some_pending = true;
                                                last_pending_id = id;
                                                precond_pending_for_this_candidate = true;
                                            }
                                            if !precond_pending_for_this_candidate {
                                                match eval_precondition(
                                                    pre, &res.agent, &res.world, ctx, None,
                                                    &bindings,
                                                ) {
                                                    StepResult::Ready(eval_res) => {
                                                        let mut cache = ctx
                                                            .discovery_precond_results
                                                            .lock()
                                                            .unwrap();
                                                        cache.insert(
                                                            (idx, pre.clone(), bindings.clone()),
                                                            eval_res,
                                                        );
                                                        if eval_res {
                                                            satisfied_preconditions.push(pre_idx);
                                                        }
                                                    }
                                                    StepResult::Pending(id) => {
                                                        let mut pending = ctx
                                                            .discovery_precond_pending
                                                            .lock()
                                                            .unwrap();
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
                                                        precond_pending_for_this_candidate = true;
                                                    }
                                                    _ => {}
                                                }
                                            }
                                        }
                                    }
                                }
                                if !precond_pending_for_this_candidate {
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
            let mut matched_req_indices = BTreeSet::new();
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
            match lookup_or_start_discovery(idx, &empty_bindings, ctx) {
                StepResult::Ready(res) => {
                    let mut satisfied_preconditions = Vec::new();
                    for (pre_idx, (_pos, pre)) in branch.open_preconditions.iter().enumerate() {
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
                                if let Some(id) = check_pending_or_clean_stale(
                                    &ctx.discovery_precond_pending,
                                    &ctx.discovery_request_map,
                                    &(idx, pre.clone(), empty_bindings.clone()),
                                ) {
                                    some_pending = true;
                                    last_pending_id = id;
                                }
                                if !some_pending {
                                    match eval_precondition(
                                        pre,
                                        &res.agent,
                                        &res.world,
                                        ctx,
                                        None,
                                        &empty_bindings,
                                    ) {
                                        StepResult::Ready(eval_res) => {
                                            let mut cache =
                                                ctx.discovery_precond_results.lock().unwrap();
                                            cache.insert(
                                                (idx, pre.clone(), empty_bindings.clone()),
                                                eval_res,
                                            );
                                            if eval_res {
                                                satisfied_preconditions.push(pre_idx);
                                            }
                                        }
                                        StepResult::Pending(id) => {
                                            let mut pending =
                                                ctx.discovery_precond_pending.lock().unwrap();
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
                    } else if branch.action_chain.is_empty() {
                        // log_debug!("Discovery: Action '{}' satisfied nothing for ROOT", action.name);
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

    CandidatesResult {
        ready: candidates,
        pending_id: if some_pending {
            Some(last_pending_id)
        } else {
            None
        },
    }
}
