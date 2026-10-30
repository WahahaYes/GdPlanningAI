//! Finds actions that satisfy a branch's open needs.
//!
//! Requirements are matched symbolically against provisions. Preconditions are matched by
//! simulating each action once against the initial state (cached in [`SearchContext`]). Discovery
//! is resumable: while a callback is outstanding, [`find_candidates`] reports its request id.

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

/// Returns the request id if `key` is still pending, removing stale entries whose answer was
/// already processed. Never holds two locks at once.
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

/// Returns the cached outcome of simulating `action_idx` once against the initial state with
/// `bindings`, starting the simulation if needed. `Pending(id)` while a callback is in flight;
/// `Invalid` when the action cannot run.
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
    /// An outstanding callback that blocked candidates (the last one seen). The engine parks the
    /// node under it.
    pub pending_id: Option<usize>,
}

/// Finds the actions that satisfy at least one open need of `branch`.
///
/// Actions failing their validity checks are skipped. Requirements are matched against
/// provisions; preconditions against each action's simulated post-state. A wildcard action
/// (GoTo) yields one candidate per matching requirement, bound to that requirement's arguments.
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
                    let mut precond_pending_for_this_candidate = false;
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
                                    precond_pending_for_this_candidate = true;
                                }
                                if !precond_pending_for_this_candidate {
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
                                            precond_pending_for_this_candidate = true;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }

                    if !precond_pending_for_this_candidate
                        && (!satisfied_requirements.is_empty()
                            || !satisfied_preconditions.is_empty())
                    {
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

// White-box: pending custom preconditions are tracked per candidate, not by a loop-wide flag. The
// state needs two evaluations in flight within one discovery pass; through the engine the host
// answers between passes, so a partial candidate never shows in a plan result.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::plan_types::{ActionSpec, CallbackKind, CallbackRequest, PreconditionSpec};
    use crate::planner::types::DiscoveryRequest;
    use crate::requirement::ProvisionSpec;
    use crate::snapshot::{BlackboardSnapshot, VariantSnapshot};
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;
    use std::sync::mpsc::{self, Receiver};

    fn empty_snapshot() -> BlackboardSnapshot {
        BlackboardSnapshot {
            properties: HashMap::new(),
            objects: HashMap::new(),
        }
    }

    fn plain_action(name: &str, provisions: Vec<ProvisionSpec>) -> ActionSpec {
        ActionSpec {
            name: name.to_string(),
            cost_callable_id: None,
            effect_callable_id: None,
            effect_spec: None,
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions,
            dependent_object_ids: vec![],
        }
    }

    /// Two non-wildcard actions sharing one pair of open custom preconditions.
    /// Only the second action satisfies the open requirement.
    fn setup() -> (Arc<SearchContext>, Receiver<CallbackRequest>) {
        let agent = empty_snapshot();
        let world = empty_snapshot();

        let actions = vec![
            plain_action("first", vec![]),
            plain_action(
                "second",
                vec![ProvisionSpec::Binding {
                    binding_name: "kit".to_string(),
                    value: VariantSnapshot::Bool(true),
                }],
            ),
        ];

        let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
        let (engine_tx, _engine_rx) = mpsc::channel();
        let ctx = Arc::new(SearchContext::new(
            actions,
            agent,
            world,
            vec![],
            req_tx,
            engine_tx,
            Arc::new(AtomicU64::new(0)),
        ));

        (ctx, req_rx)
    }

    fn open_branch(ctx: &SearchContext) -> PlanBranch {
        let mut branch = PlanBranch::new(
            Arc::clone(&ctx.initial_agent),
            Arc::clone(&ctx.initial_world),
        );
        branch.open_requirements = vec![(
            0,
            RequirementSpec::BindingExists {
                binding_name: "kit".to_string(),
            },
        )];
        branch.open_preconditions = vec![
            (
                0,
                PreconditionSpec::Custom {
                    callable_id: 101,
                    dependent_object_ids: vec![],
                },
            ),
            (
                0,
                PreconditionSpec::Custom {
                    callable_id: 102,
                    dependent_object_ids: vec![],
                },
            ),
        ];
        branch
    }

    fn drain_eval_requests(rx: &Receiver<CallbackRequest>) -> Vec<CallbackRequest> {
        let mut out = Vec::new();
        while let Ok(req) = rx.try_recv() {
            assert!(
                matches!(req.kind, CallbackKind::EvalCustomPrecond { .. }),
                "only custom-precondition callbacks expected"
            );
            out.push(req);
        }
        out
    }

    #[test]
    fn later_action_evaluates_own_preconditions_after_first_goes_pending() {
        let (ctx, req_rx) = setup();
        let branch = open_branch(&ctx);

        let result = find_candidates(&branch, &ctx);

        // The second action's custom precondition evaluation must not be skipped
        // just because the first action went pending: both actions fire callbacks.
        let eval_reqs = drain_eval_requests(&req_rx);
        assert_eq!(
            eval_reqs.len(),
            2,
            "each action should evaluate its own custom preconditions (one in-flight each)"
        );
        let pending_actions: Vec<usize> = {
            let req_map = ctx.discovery_request_map.lock().unwrap();
            req_map
                .values()
                .filter_map(|r| match r {
                    DiscoveryRequest::Precondition(idx, _, _) => Some(*idx),
                    _ => None,
                })
                .collect()
        };
        assert!(
            pending_actions.contains(&0),
            "first action should have a pending precondition"
        );
        assert!(
            pending_actions.contains(&1),
            "second action's custom precondition must be evaluated even though the \
             first action went pending"
        );

        // Neither action may be emitted with a partial satisfied_preconditions list
        // while its own precondition evaluation is still in flight.
        assert!(
            result.ready.is_empty(),
            "no candidate may be emitted while its preconditions are pending, \
             got: {:?}",
            result
                .ready
                .iter()
                .map(|c| (c.action_idx, c.satisfied_preconditions.clone()))
                .collect::<Vec<_>>()
        );
        assert!(
            result.pending_id.is_some(),
            "pending discovery must be reported"
        );
    }

    #[test]
    fn pending_preconditions_expand_once_resolved() {
        let (ctx, req_rx) = setup();
        let branch = open_branch(&ctx);

        let first = find_candidates(&branch, &ctx);
        assert!(first.ready.is_empty());
        assert!(first.pending_id.is_some());
        drop(drain_eval_requests(&req_rx));

        // Resolve in-flight precondition callbacks as true until discovery
        // settles, the way the engine re-runs discovery as answers arrive. Each
        // pass resolves one precondition per action: a later precondition only
        // fires once the earlier one is cached.
        let mut settled = None;
        for _ in 0..4 {
            let res = find_candidates(&branch, &ctx);
            drop(drain_eval_requests(&req_rx));
            let keys: Vec<_> = {
                ctx.discovery_precond_pending
                    .lock()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect()
            };
            if keys.is_empty() {
                settled = Some(res);
                break;
            }
            {
                let mut results = ctx.discovery_precond_results.lock().unwrap();
                for key in &keys {
                    results.insert(key.clone(), true);
                }
            }
            ctx.discovery_precond_pending.lock().unwrap().clear();
            ctx.discovery_request_map.lock().unwrap().clear();
            settled = Some(res);
        }
        let second = settled.expect("discovery should settle once callbacks resolve");

        // No further callbacks: everything is cached now.
        assert!(
            req_rx.try_recv().is_err(),
            "resolved preconditions must not re-fire callbacks"
        );

        let cand = second
            .ready
            .iter()
            .find(|c| c.action_idx == 1)
            .expect("second action should expand once its preconditions resolve");
        assert_eq!(cand.satisfied_preconditions, vec![0, 1]);
        assert_eq!(cand.satisfied_requirements.len(), 1);
        assert!(second.pending_id.is_none());
    }
}
