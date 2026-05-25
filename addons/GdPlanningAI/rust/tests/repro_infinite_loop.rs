use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::{
    PlannerEngine, SearchAlgorithm, SearchContext, TerminationStrategy,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::time::{Duration, Instant};

#[test]
fn test_verifying_terminal_state() {
    let mut props = HashMap::new();
    props.insert("goal_met".to_string(), VariantSnapshot::Bool(false));
    let agent = BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    // Action that meets the goal
    let actions = vec![ActionSpec {
        name: "finish".to_string(),
        cost_callable_id: None,
        effect_callable_id: Some(1),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "goal".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    // Mock responder that makes the action satisfy the goal
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    std::thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    agent
                        .properties
                        .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
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

    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let cancel_flag = Arc::new(AtomicBool::new(false));
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
    });

    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag)
        .with_search_algorithm(SearchAlgorithm::AStar)
        .with_termination_strategy(TerminationStrategy::FirstComplete);
    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start_time = Instant::now();
    let timeout = Duration::from_secs(2);
    let mut total_iterations = 0;

    loop {
        if start_time.elapsed() > timeout {
            panic!(
                "Test timed out! Likely infinite loop in Verifying state. Total iterations: {}",
                total_iterations
            );
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => {
                assert!(res.is_some(), "Should have found a plan");
                assert!(res.unwrap().success, "Plan should be successful");
                break;
            }
            PlannerRunResult::Pending(_) => {
                total_iterations += 1;
                std::thread::sleep(Duration::from_millis(1));
            }
        }
    }
}
