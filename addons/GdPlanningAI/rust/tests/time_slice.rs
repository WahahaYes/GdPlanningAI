//! Regression test for wall-clock time-slicing preserving plan results.
//!
//! Verifies that planning with time-slicing disabled (`time_slice_ms = 0`)
//! and enabled (1ms slice) yields the same final plan.

use gdplanningai_rust::plan_tree::PlanResult;
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
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn make_agent() -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(80));
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

fn make_world() -> BlackboardSnapshot {
    BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    }
}

fn make_actions_with_count(count: usize) -> Vec<ActionSpec> {
    // Identical candidates to force >128 queue pops so the
    // wall-clock slice gate (checked every 128 pops) is exercised.
    // Each clone has distinct name but same cost/effect behavior.
    (0..count)
        .map(|i| ActionSpec {
            name: format!("eat_{}", i),
            cost_callable_id: Some(0),
            effect_callable_id: Some(0),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        })
        .collect()
}

fn make_goals() -> Vec<GoalSpec> {
    vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(30)),
        }],
        original_index: 0,
    }]
}

fn spawn_callback_responder() -> mpsc::Sender<CallbackRequest> {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(5.0),
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    if let Some(VariantSnapshot::Int(current)) =
                        agent.properties.get("hunger").cloned()
                    {
                        let new_hunger = (current - 60).max(0);
                        agent
                            .properties
                            .insert("hunger".to_string(), VariantSnapshot::Int(new_hunger));
                    }
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                _ => CallbackResponse::Float(1.0),
            };
            let _ = req.response_tx.send(PlannerCallback {
                request_id: req.request_id,
                response,
            });
        }
    });
    req_tx
}

fn run_to_completion(time_slice_ms: u64, action_count: usize) -> (Option<PlanResult>, usize) {
    let agent = make_agent();
    let world = make_world();
    let actions = make_actions_with_count(action_count);
    let goals = make_goals();
    let request_tx = spawn_callback_responder();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions: Vec<usize> = (0..actions.len()).collect();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions: vec![],
        request_tx,
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

    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag)
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost)
        .with_time_slice_ms(time_slice_ms);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start = Instant::now();
    let timeout = Duration::from_secs(15);
    let mut pending_zero_count = 0usize;
    loop {
        if start.elapsed() > timeout {
            panic!(
                "run_to_completion({}): timed out waiting for plan completion",
                time_slice_ms
            );
        }
        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => return (res, pending_zero_count),
            PlannerRunResult::Pending(0) => {
                pending_zero_count += 1;
                thread::sleep(Duration::from_millis(1))
            }
            PlannerRunResult::Pending(_) => thread::sleep(Duration::from_millis(1)),
        }
    }
}

#[test]
fn disabled_vs_enabled_yields_same_plan() {
    let (disabled, disabled_pending) = run_to_completion(0, 300);
    let (enabled, enabled_pending) = run_to_completion(1, 300);

    let disabled = disabled.expect("disabled slice should produce a plan");
    let enabled = enabled.expect("enabled slice should produce a plan");

    assert!(disabled.success, "disabled slice should succeed");
    assert!(enabled.success, "enabled slice should succeed");
    assert_eq!(
        disabled.action_chain.len(),
        enabled.action_chain.len(),
        "time-slicing must not change plan length"
    );
    assert_eq!(
        disabled.total_cost, enabled.total_cost,
        "time-slicing must not change the cost"
    );
    assert_eq!(disabled.action_chain.len(), 1);
    assert_eq!(disabled.total_cost, 5.0);
    assert_eq!(
        disabled_pending, 0,
        "disabled slice should never yield Pending(0) via timer"
    );
    assert!(
        enabled_pending > 0,
        "enabled 1ms slice on 300-candidate fan-out should yield Pending(0) at least once, got 0"
    );
}

#[test]
fn small_pop_never_yields_to_timer() {
    // Fewer than 128 queue pops: the wall-clock gate (checked every 128
    // pops) must never fire, even with a 1ms slice enabled.
    let (plan, pending_zero_count) = run_to_completion(1, 5);

    let plan = plan.expect("small fan-out should produce a plan");
    assert!(plan.success, "small fan-out should succeed");
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.total_cost, 5.0);
    assert_eq!(
        pending_zero_count, 0,
        "fewer than 128 pops must never yield Pending(0) via timer"
    );
}

#[test]
fn cancel_returns_complete_immediately() {
    // Cancel flag set before planning must win over time-slice yield:
    // plan() should return Complete(None), never Pending(0).
    let agent = make_agent();
    let world = make_world();
    let actions = make_actions_with_count(300);
    let goals = make_goals();
    let request_tx = spawn_callback_responder();

    let cancel_flag = Arc::new(AtomicBool::new(true));
    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions: Vec<usize> = (0..actions.len()).collect();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions: vec![],
        request_tx,
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

    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag)
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost)
        .with_time_slice_ms(1);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    match engine.plan(&goals) {
        PlannerRunResult::Complete(None) => {}
        other => panic!("cancel should return Complete(None), got {:?}", other),
    }
}
