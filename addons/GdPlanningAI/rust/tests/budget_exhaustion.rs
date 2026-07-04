//! Regression test for search-budget exhaustion yielding Pending instead of Complete.
//!
//! Test Gap #7: Verifies that when the iteration budget is exhausted before a
//! complete plan can be found, the planner returns `PlannerRunResult::Pending`
//! rather than `Complete`.

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

#[test]
fn iteration_budget_exhaustion_returns_pending() {
    // Goal requires hunger < 30. Agent hunger is 80.
    // One action "eat" reduces hunger by 60.
    // With iteration_budget = 0, the very first node popped from the queue
    // should trigger budget exhaustion and return Pending(0).
    let agent = make_agent();
    let world = make_world();

    let actions = vec![ActionSpec {
        name: "eat".to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(0),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(30)),
        }],
        original_index: 0,
    }];

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions: Vec<usize> = (0..actions.len()).collect();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions: vec![],
        request_tx: mpsc::channel().0, // dummy sender; not used in this test
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
        .with_iteration_budget(0);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let result = engine.plan(&goals);
    assert!(
        matches!(result, PlannerRunResult::Pending(0)),
        "Expected Pending(0) when iteration budget is exhausted, got {:?}",
        result
    );
}

#[test]
fn max_depth_prevents_plan_completion() {
    // Goal requires hunger < 30. Agent hunger is 80.
    // Action "eat" has precondition "has_food".
    // Action "get_food" satisfies "has_food".
    // With max_depth = 1, the planner can only prepend one action,
    // so it cannot find a complete 2-action plan. It should exhaust
    // the search and return a failed Complete result (not panic or hang).
    let agent = make_agent();
    let world = make_world();

    let actions = vec![
        ActionSpec {
            name: "eat".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(0),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::HasProperty,
                property_name: "has_food".to_string(),
                value: None,
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "get_food".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(1),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(30)),
        }],
        original_index: 0,
    }];

    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    let handle = std::thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(1.0),
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    match req.callable_id {
                        0 => {
                            // eat effect
                            if let Some(VariantSnapshot::Int(current)) =
                                agent.properties.get("hunger").cloned()
                            {
                                let new_hunger = (current - 60).max(0);
                                agent
                                    .properties
                                    .insert("hunger".to_string(), VariantSnapshot::Int(new_hunger));
                            }
                        }
                        1 => {
                            // get_food effect
                            agent
                                .properties
                                .insert("has_food".to_string(), VariantSnapshot::Bool(true));
                        }
                        _ => {}
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

    let mut engine = PlannerEngine::new(ctx, 1, cancel_flag)
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let mut iterations = 0;
    let result = loop {
        iterations += 1;
        assert!(
            iterations < 1000,
            "Planner looped too many times without converging"
        );
        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => break res,
            PlannerRunResult::Pending(_) => {}
        }
    };

    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(
        !plan.success,
        "Expected failure when max_depth=1 blocks 2-action plan"
    );
}
