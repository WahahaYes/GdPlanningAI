//! Integration tests for the backward-chaining planner.
//!
//! These tests drive a `PlannerEngine` end-to-end (via the `run_planner` helper below) with a
//! mock callback responder thread standing in for the Godot main thread, verifying that the
//! planner correctly chains actions to satisfy goals.

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::types::{PriorityNode, SearchNode};
use gdplanningai_rust::planner::{
    DijkstraHeuristic, PlanBranch, PlannerEngine, SearchContext, TerminationStrategy,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

fn make_agent(hunger: i64) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(hunger));
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

fn hunger_less_than(threshold: i64) -> PreconditionSpec {
    PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::LessThan,
        property_name: "hunger".to_string(),
        value: Some(VariantSnapshot::Int(threshold)),
    }
}

/// Spawn a thread that responds to callback requests from the planner.
/// Returns the sender to give to the planner and a join handle.
fn spawn_callback_responder(
    cost_value: f64,
    hunger_reduction: i64,
) -> (mpsc::Sender<CallbackRequest>, thread::JoinHandle<()>) {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();

    let handle = thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(cost_value),
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    if let Some(VariantSnapshot::Int(current)) =
                        agent.properties.get("hunger").cloned()
                    {
                        let new_hunger = (current - hunger_reduction).max(0);
                        agent
                            .properties
                            .insert("hunger".to_string(), VariantSnapshot::Int(new_hunger));
                    }
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { .. } => CallbackResponse::Bool(true),
            };
            let _ = req
                .response_tx
                .send(gdplanningai_rust::plan_types::PlannerCallback {
                    request_id: req.request_id,
                    response,
                });
        }
    });

    (req_tx, handle)
}

fn run_planner(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    max_depth: usize,
    request_tx: mpsc::Sender<CallbackRequest>,
) -> Option<PlanResult> {
    let cancel_flag = Arc::new(AtomicBool::new(false));

    // Initialize logging for tests
    gdplanningai_rust::logger::init_log_channel();
    gdplanningai_rust::logger::set_log_level(gdplanningai_rust::logger::LogLevel::Debug);

    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions: Vec<usize> = (0..actions.len()).collect();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent.into(),
        initial_world: world.into(),
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

    let mut engine = PlannerEngine::new(ctx, max_depth, cancel_flag)
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost);

    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start_time = Instant::now();
    let timeout = Duration::from_secs(5);

    loop {
        if start_time.elapsed() > timeout {
            panic!("run_planner: Timed out waiting for plan completion");
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => return res,
            PlannerRunResult::Pending(_) => {
                // In this test setup, the responder thread is already running
                // and should be sending responses to engine.response_rx.
                // We just need to give it a tiny bit of time to breathe.
                thread::sleep(Duration::from_millis(1));
            }
        }
    }
}

#[test]
fn single_action_satisfies_goal() {
    // Agent is hungry (80), goal is hunger < 30.
    // One action "eat" reduces hunger by 60 with cost 5.
    // Planner should find the 1-action plan.
    let agent = make_agent(80);
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    // Drop the sender so the responder thread exits
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 0);
    assert_eq!(plan.total_cost, 5.0);
}

#[test]
fn goal_already_satisfied_returns_empty_plan() {
    // Agent hunger is 10, goal is hunger < 30.
    // Goal is already met — planner should return empty plan with zero cost.
    let agent = make_agent(10);
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert!(plan.action_chain.is_empty());
    assert_eq!(plan.total_cost, 0.0);
}

#[test]
fn no_valid_plan_returns_failure() {
    // Agent is hungry (80), goal is hunger < 30.
    // No actions available — planner should return failure.
    let agent = make_agent(80);
    let world = make_world();
    let actions = vec![];
    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn action_with_unmet_precondition_fails() {
    // Agent hunger is 80, goal is hunger < 30.
    // Action "eat" has a precondition "has_food" which is not in the agent state.
    // Planner should find no valid plan.
    let agent = make_agent(80);
    let world = make_world();

    let actions = vec![ActionSpec {
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
    }];

    let goals = vec![GoalSpec {
        name: "satisfy_hunger".to_string(),
        reward: 10.0,
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn respects_max_depth() {
    // Agent hunger is 80, goal is hunger < 30.
    // Action "eat" reduces hunger but has a precondition "has_food".
    // Action "get_food" provides "has_food" via its effect.
    // With max_depth=0, no actions can be added to the chain → failure.
    let agent = make_agent(80);
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
            effect_callable_id: Some(0),
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    // max_depth=0: no actions allowed in the chain
    let result = run_planner(agent, world, actions, goals, 0, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success);
}

#[test]
fn goal_priority_respected() {
    // Two goals: high-priority "hunger < 30" (reward 100) and low-priority
    // "hunger < 80" (reward 1). The high-priority goal requires an action
    // with a precondition that can't be met, but the low-priority goal is
    // already satisfied. The planner should return the low-priority goal
    // as satisfied (empty plan) since the high-priority one fails.
    let agent = make_agent(50); // hunger=50: satisfies <80 but not <30
    let world = make_world();

    let actions = vec![ActionSpec {
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
    }];

    let goals = vec![
        GoalSpec {
            name: "starving".to_string(),
            reward: 100.0,
            desired_state: vec![hunger_less_than(30)],
            original_index: 0,
        },
        GoalSpec {
            name: "peckish".to_string(),
            reward: 1.0,
            desired_state: vec![hunger_less_than(80)],
            original_index: 1,
        },
    ];

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // The "peckish" goal (hunger < 80) is already satisfied
    assert_eq!(plan.goal_index, 1);
    assert!(plan.action_chain.is_empty());
}

fn agent_int_equal(property: &str, value: i64) -> PreconditionSpec {
    PreconditionSpec::Builtin {
        target: PreconditionTarget::Agent,
        operation: PreconditionOp::Equal,
        property_name: property.to_string(),
        value: Some(VariantSnapshot::Int(value)),
    }
}

fn int_effect_action(name: &str, effect_id: usize, pre: Vec<PreconditionSpec>) -> ActionSpec {
    ActionSpec {
        name: name.to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(effect_id),
        preconditions: pre,
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }
}

/// Answers callbacks for actions whose effects set agent ints: `effects[callable_id]` lists the
/// `(property, value)` pairs an effect writes, and `customs[callable_id]` is the
/// `(property, value)` a custom precondition requires. Costs are 1.0.
fn spawn_int_responder(
    effects: Vec<Vec<(&'static str, i64)>>,
    customs: HashMap<usize, (&'static str, i64)>,
) -> (mpsc::Sender<CallbackRequest>, thread::JoinHandle<()>) {
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    let handle = thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::GetCost { .. } => CallbackResponse::Float(1.0),
                CallbackKind::ApplyEffect { mut agent, world } => {
                    for &(prop, value) in &effects[req.callable_id] {
                        agent
                            .properties
                            .insert(prop.to_string(), VariantSnapshot::Int(value));
                    }
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { agent, .. } => {
                    let (prop, value) = customs[&req.callable_id];
                    CallbackResponse::Bool(
                        agent.properties.get(prop) == Some(&VariantSnapshot::Int(value)),
                    )
                }
            };
            let _ = req.response_tx.send(PlannerCallback {
                request_id: req.request_id,
                response,
            });
        }
    });
    (req_tx, handle)
}

#[test]
fn goal_level_action_runs_after_the_chain() {
    // Goal: fed and at the fire. Eating needs the agent at the food, so walking to the fire
    // must come last: [goto_food, eat, goto_fire]. Effects set one agent int each.
    let (req_tx, handle) = spawn_int_responder(
        vec![vec![("pos", 1)], vec![("fed", 1)], vec![("pos", 2)]],
        HashMap::new(),
    );

    let mut agent = make_agent(0);
    agent
        .properties
        .insert("pos".to_string(), VariantSnapshot::Int(0));
    let actions = vec![
        int_effect_action("goto_food", 0, vec![]),
        int_effect_action("eat", 1, vec![agent_int_equal("pos", 1)]),
        int_effect_action("goto_fire", 2, vec![]),
    ];
    let goals = vec![GoalSpec {
        name: "fed_by_fire".to_string(),
        reward: 10.0,
        desired_state: vec![agent_int_equal("fed", 1), agent_int_equal("pos", 2)],
        original_index: 0,
    }];

    let result = run_planner(agent, make_world(), actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain, vec![0, 1, 2]);
}

#[test]
fn custom_precondition_is_rechecked_before_its_action() {
    // Eating needs the agent at the food (builtin) and holding food (custom). Walking to the food
    // drops the held item, so picking up must come after it: [goto_food, pickup, eat].
    let mut agent = make_agent(0);
    for prop in ["pos", "held"] {
        agent
            .properties
            .insert(prop.to_string(), VariantSnapshot::Int(0));
    }
    let holding = PreconditionSpec::Custom {
        callable_id: 3,
        dependent_object_ids: vec![],
    };
    let actions = vec![
        int_effect_action("goto_food", 0, vec![]),
        int_effect_action("pickup", 1, vec![]),
        int_effect_action("eat", 2, vec![agent_int_equal("pos", 1), holding]),
    ];
    let goals = vec![GoalSpec {
        name: "fed".to_string(),
        reward: 10.0,
        desired_state: vec![agent_int_equal("fed", 1)],
        original_index: 0,
    }];
    let (req_tx, handle) = spawn_int_responder(
        vec![
            vec![("pos", 1), ("held", 0)],
            vec![("held", 1)],
            vec![("fed", 1), ("held", 0)],
        ],
        HashMap::from([(3, ("held", 1))]),
    );

    let result = run_planner(agent, make_world(), actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain, vec![0, 1, 2]);
}

#[test]
fn plan_breaking_a_custom_precondition_is_rejected() {
    // As above, but picking up also walks the agent home, so no order has it at the food and
    // holding food at once. [pickup, goto_food, eat] satisfies each need once; goto_food then
    // drops the item, which only re-checking the custom precondition catches.
    let mut agent = make_agent(0);
    for prop in ["pos", "held"] {
        agent
            .properties
            .insert(prop.to_string(), VariantSnapshot::Int(0));
    }
    let holding = PreconditionSpec::Custom {
        callable_id: 3,
        dependent_object_ids: vec![],
    };
    let actions = vec![
        int_effect_action("goto_food", 0, vec![]),
        int_effect_action("pickup", 1, vec![]),
        int_effect_action("eat", 2, vec![agent_int_equal("pos", 1), holding]),
    ];
    let goals = vec![GoalSpec {
        name: "fed".to_string(),
        reward: 10.0,
        desired_state: vec![agent_int_equal("fed", 1)],
        original_index: 0,
    }];
    let (req_tx, handle) = spawn_int_responder(
        vec![
            vec![("pos", 1), ("held", 0)],
            vec![("pos", 0), ("held", 1)],
            vec![("fed", 1), ("held", 0)],
        ],
        HashMap::from([(3, ("held", 1))]),
    );

    let result = run_planner(agent, make_world(), actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a result");
    assert!(!plan.success, "got {:?}", plan.action_chain);
}

#[test]
fn goal_switch_drops_nodes_queued_under_the_previous_goal() {
    // Goal 0 holds already (empty-chain plan at cost 0), and a goal-0 node is still queued, as
    // the BestCost optimality break leaves it. Switching to goal 1 must drop that node rather
    // than verify it against goal 1's desired state.
    let agent: Arc<BlackboardSnapshot> = make_agent(80).into();
    let world: Arc<BlackboardSnapshot> = make_world().into();
    let goals = vec![
        GoalSpec {
            name: "already_met".to_string(),
            reward: 10.0,
            desired_state: vec![hunger_less_than(100)],
            original_index: 0,
        },
        GoalSpec {
            name: "satisfy_hunger".to_string(),
            reward: 5.0,
            desired_state: vec![hunger_less_than(30)],
            original_index: 1,
        },
    ];

    let (request_tx, _request_rx) = mpsc::channel::<CallbackRequest>();
    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let ctx = Arc::new(SearchContext {
        actions: vec![],
        initial_agent: Arc::clone(&agent),
        initial_world: Arc::clone(&world),
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
        non_wildcard_actions: vec![],
    });
    let mut engine = PlannerEngine::new(ctx, 10, Arc::new(AtomicBool::new(false)))
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost);
    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    engine.best_plan = Some(PlanResult {
        success: true,
        action_chain: vec![],
        total_cost: 0.0,
        goal_index: 0,
        action_bindings: vec![],
    });
    engine.best_cost = 0.0;
    let mut stale = PlanBranch::new(agent, world);
    stale.goal_index = 0;
    stale.cost = 5.0;
    engine.queue.push(PriorityNode {
        priority: 5.0,
        node: SearchNode {
            branch: stale,
            resumed: false,
            callback_response: None,
            expanded_candidates: vec![],
        },
    });

    assert!(matches!(engine.plan(&goals), PlannerRunResult::Pending(0)));
    assert_eq!(engine.current_goal_index, 1);
    assert_eq!(engine.queue.len(), 1, "only goal 1's root should be queued");
    assert_eq!(engine.queue.peek().unwrap().node.branch.goal_index, 1);
}

#[test]
fn verification_reuses_the_discovered_first_step() {
    // Discovery already simulated eat from the initial state, so verifying the one-step plan
    // reuses that post-state: the effect callback fires once, not twice.
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
        desired_state: vec![hunger_less_than(30)],
        original_index: 0,
    }];

    let (responder_tx, _handle) = spawn_callback_responder(5.0, 60);
    let effect_calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    let counter = Arc::clone(&effect_calls);
    thread::spawn(move || {
        for req in req_rx {
            if matches!(req.kind, CallbackKind::ApplyEffect { .. }) {
                counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            }
            let _ = responder_tx.send(req);
        }
    });

    let plan = run_planner(make_agent(80), make_world(), actions, goals, 10, req_tx)
        .expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain, vec![0]);
    assert_eq!(
        effect_calls.load(std::sync::atomic::Ordering::Relaxed),
        1,
        "verification should reuse discovery's simulation of step 0"
    );
}
