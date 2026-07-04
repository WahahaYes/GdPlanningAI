//! Integration tests for TerminationStrategy::BestCost vs FirstComplete.

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

/// Run the planner with the given termination strategy and return the plan result.
fn run_with_strategy(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    strategy: TerminationStrategy,
    request_tx: mpsc::Sender<CallbackRequest>,
) -> Option<PlanResult> {
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
        .with_termination_strategy(strategy);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);

    loop {
        if start_time.elapsed() > timeout {
            panic!("run_with_strategy: Timed out waiting for plan completion");
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => return res,
            PlannerRunResult::Pending(_) => {
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

#[test]
fn best_cost_returns_lowest_cost_plan() {
    // Chain A: one action (quick) with cost 1.0, directly satisfies goal.
    // Chain B: two actions (slow1 + slow2) with total cost 5.0, also satisfies goal.
    // BestCost should return Chain A.

    let agent = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    let actions = vec![
        ActionSpec {
            name: "quick".to_string(),
            cost_callable_id: Some(10),
            effect_callable_id: Some(100),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow1".to_string(),
            cost_callable_id: Some(20),
            effect_callable_id: Some(101),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow2".to_string(),
            cost_callable_id: Some(30),
            effect_callable_id: Some(102),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::Equal,
                property_name: "can_finish".to_string(),
                value: Some(VariantSnapshot::Bool(true)),
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "finish".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => {
                    let cost = match req.callable_id {
                        10 => 1.0,
                        20 => 2.0,
                        30 => 3.0,
                        _ => 1.0,
                    };
                    CallbackResponse::Float(cost)
                }
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    match req.callable_id {
                        100 => {
                            agent.properties.insert(
                                "goal_met".to_string(),
                                VariantSnapshot::Bool(true),
                            );
                        }
                        101 => {
                            agent.properties.insert(
                                "can_finish".to_string(),
                                VariantSnapshot::Bool(true),
                            );
                        }
                        102 => {
                            agent.properties.insert(
                                "goal_met".to_string(),
                                VariantSnapshot::Bool(true),
                            );
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

    let result = run_with_strategy(
        agent, world, actions, goals, TerminationStrategy::BestCost, req_tx,
    );

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // Chain A (quick) is the cheapest valid plan at cost 1.0
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0); // quick
    assert_eq!(plan.total_cost, 1.0);
}

#[test]
fn first_complete_returns_first_found_plan() {
    // Same setup as best_cost test.
    // With Dijkstra heuristic, the cheapest node is always expanded first,
    // so FirstComplete also finds Chain A (quick, cost 1.0) first.
    // The test documents this behavior.

    let agent = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };

    let actions = vec![
        ActionSpec {
            name: "quick".to_string(),
            cost_callable_id: Some(10),
            effect_callable_id: Some(100),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow1".to_string(),
            cost_callable_id: Some(20),
            effect_callable_id: Some(101),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "slow2".to_string(),
            cost_callable_id: Some(30),
            effect_callable_id: Some(102),
            preconditions: vec![PreconditionSpec::Builtin {
                target: PreconditionTarget::Agent,
                operation: PreconditionOp::Equal,
                property_name: "can_finish".to_string(),
                value: Some(VariantSnapshot::Bool(true)),
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "finish".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "goal_met".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => {
                    let cost = match req.callable_id {
                        10 => 1.0,
                        20 => 2.0,
                        30 => 3.0,
                        _ => 1.0,
                    };
                    CallbackResponse::Float(cost)
                }
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    match req.callable_id {
                        100 => {
                            agent.properties.insert(
                                "goal_met".to_string(),
                                VariantSnapshot::Bool(true),
                            );
                        }
                        101 => {
                            agent.properties.insert(
                                "can_finish".to_string(),
                                VariantSnapshot::Bool(true),
                            );
                        }
                        102 => {
                            agent.properties.insert(
                                "goal_met".to_string(),
                                VariantSnapshot::Bool(true),
                            );
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

    let result = run_with_strategy(
        agent, world, actions, goals, TerminationStrategy::FirstComplete, req_tx,
    );

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // With Dijkstra heuristic the cheapest complete branch is expanded first,
    // so FirstComplete also returns Chain A (quick, cost 1.0).
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0); // quick
    assert_eq!(plan.total_cost, 1.0);
}
