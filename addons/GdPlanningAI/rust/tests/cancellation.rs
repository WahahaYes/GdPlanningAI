//! Integration test for planner cancellation via cancel_flag.

use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::{
    DijkstraHeuristic, PlannerEngine, SearchContext, TerminationStrategy,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn cancellation_returns_failure_quickly() {
    // Set up a planner with an action that has a custom validity check.
    // The custom check fires a callback that the responder answers slowly.
    // This parks the root node and keeps the search in Pending state,
    // giving us time to cancel.
    let agent = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    let actions = vec![ActionSpec {
        name: "slow_action".to_string(),
        cost_callable_id: Some(1),
        effect_callable_id: Some(2),
        preconditions: vec![],
        validity_checks: vec![PreconditionSpec::Custom {
            callable_id: 99,
            dependent_object_ids: vec![],
        }],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "impossible".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "magic".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    // Spawn a slow responder so the callback stays pending for a while.
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            thread::sleep(Duration::from_millis(100));
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(1.0),
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    agent
                        .properties
                        .insert("magic".to_string(), VariantSnapshot::Bool(true));
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(true),
            };
            let _ = req.response_tx.send(PlannerCallback {
                request_id: req.request_id,
                response,
            });
        }
    });

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions: Vec<usize> = (0..actions.len()).collect();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions: vec![],
        request_tx: req_tx,
        engine_response_tx: engine_tx.clone(),
        discovery_results: std::sync::Mutex::new(HashMap::new()),
        discovery_costs: std::sync::Mutex::new(HashMap::new()),
        discovery_pending: std::sync::Mutex::new(HashMap::new()),
        discovery_request_map: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_results: std::sync::Mutex::new(HashMap::new()),
        discovery_precond_pending: std::sync::Mutex::new(HashMap::new()),
        provision_index: HashMap::new(),
        non_wildcard_actions,
    });

    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag.clone())
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start_time = Instant::now();
    let mut iterations = 0;
    let mut saw_pending = false;
    loop {
        if iterations >= 3 {
            cancel_flag.store(true, Ordering::Relaxed);
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => {
                if cancel_flag.load(Ordering::Relaxed) {
                    assert!(
                        res.is_none(),
                        "Planner should have returned None after cancellation"
                    );
                    break;
                }
                // If we complete before cancelling, something went wrong
                panic!("Planner completed before cancellation could take effect");
            }
            PlannerRunResult::Pending(_) => {
                saw_pending = true;
                iterations += 1;
                thread::sleep(Duration::from_millis(5));
            }
        }
    }

    let elapsed = start_time.elapsed();
    assert!(saw_pending, "Planner should have entered Pending state");
    assert!(
        elapsed < Duration::from_secs(2),
        "Cancellation took too long: {:?}",
        elapsed
    );
}
