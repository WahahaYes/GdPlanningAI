//! Regression test: the non-wildcard path in [`find_candidates`] must track
//! pending custom-precondition evaluations per-candidate, not via a loop-wide
//! flag shared across actions.
//!
//! With two non-wildcard actions where the first action's custom precondition
//! goes pending, the second action must still evaluate its own custom
//! preconditions (firing its own callback) and must not be emitted with a
//! partial `satisfied_preconditions` list. Once the callbacks resolve, the
//! second action expands with its full precondition set.

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, PlannerCallback, PreconditionSpec,
};
use gdplanningai_rust::planner::expander::find_candidates;
use gdplanningai_rust::planner::types::{DiscoveryRequest, PlanBranch, SearchContext};
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
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
    let (engine_tx, _engine_rx) = mpsc::channel::<PlannerCallback>();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions: vec![],
        request_tx: req_tx,
        engine_response_tx: engine_tx,
        discovery_results: std::sync::Mutex::new(HashMap::new()),
        discovery_costs: std::sync::Mutex::new(HashMap::new()),
        discovery_pending: std::sync::Mutex::new(HashMap::new()),
        discovery_request_map: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_results: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_pending: std::sync::Mutex::new(HashMap::new()),
        provision_index: HashMap::from([(
            (
                gdplanningai_rust::planner::ProvisionKind::Binding,
                "kit".to_string(),
            ),
            vec![1usize],
        )]),
        non_wildcard_actions: vec![0, 1],
    });

    (ctx, req_rx)
}

fn open_branch(ctx: &SearchContext) -> PlanBranch {
    let mut branch = PlanBranch::new(&ctx.initial_agent, &ctx.initial_world);
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
