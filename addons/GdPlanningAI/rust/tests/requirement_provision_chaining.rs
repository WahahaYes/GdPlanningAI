//! Integration tests for requirement/provision chaining, wildcard bindings,
//! custom preconditions, and BindingInSet with world group context.

mod common;
use common::{create_sim_object, create_test_agent, create_test_world};
use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::types::ProvisionKind;
use gdplanningai_rust::planner::{
    DijkstraHeuristic, PlannerEngine, SearchContext, TerminationStrategy,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

// ── Helpers ────────────────────────────────────────────────────────

fn build_provision_index(actions: &[ActionSpec]) -> HashMap<(ProvisionKind, String), Vec<usize>> {
    let mut index: HashMap<(ProvisionKind, String), Vec<usize>> = HashMap::new();
    for (idx, action) in actions.iter().enumerate() {
        for prov in &action.provisions {
            let (kind, name) = match prov {
                ProvisionSpec::Binding { binding_name, .. } => {
                    (ProvisionKind::Binding, binding_name.clone())
                }
                ProvisionSpec::Fact { fact_name, .. } => (ProvisionKind::Fact, fact_name.clone()),
                ProvisionSpec::FactWildcard { fact_name } => {
                    (ProvisionKind::FactWildcard, fact_name.clone())
                }
            };
            index.entry((kind, name)).or_default().push(idx);
        }
    }
    index
}

fn build_non_wildcard_actions(actions: &[ActionSpec]) -> Vec<usize> {
    actions
        .iter()
        .enumerate()
        .filter(|(_, a)| {
            !a.provisions
                .iter()
                .any(|p| matches!(p, ProvisionSpec::FactWildcard { .. }))
        })
        .map(|(i, _)| i)
        .collect()
}

#[allow(dead_code)]
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
                    if let Some(VariantSnapshot::Bool(_)) =
                        agent.properties.get("goal_met").cloned()
                    {
                        agent
                            .properties
                            .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
                    }
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

    (req_tx, handle)
}

/// Spawn a responder that only reduces hunger for a specific effect callable.
fn spawn_action_aware_responder(
    cost_value: f64,
    hunger_effect_callable_id: usize,
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
                    if req.callable_id == hunger_effect_callable_id {
                        if let Some(VariantSnapshot::Int(current)) =
                            agent.properties.get("hunger").cloned()
                        {
                            let new_hunger = (current - hunger_reduction).max(0);
                            agent
                                .properties
                                .insert("hunger".to_string(), VariantSnapshot::Int(new_hunger));
                        }
                    }
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

    (req_tx, handle)
}

/// Spawn a responder that returns false for a specific callable_id.
fn spawn_selective_callback_responder(
    cost_value: f64,
    hunger_reduction: i64,
    false_callable_id: usize,
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
                    if let Some(VariantSnapshot::Bool(_)) =
                        agent.properties.get("goal_met").cloned()
                    {
                        agent
                            .properties
                            .insert("goal_met".to_string(), VariantSnapshot::Bool(true));
                    }
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { .. } => {
                    if req.callable_id == false_callable_id {
                        CallbackResponse::Bool(false)
                    } else {
                        CallbackResponse::Bool(true)
                    }
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

fn run_planner(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    max_depth: usize,
    request_tx: mpsc::Sender<CallbackRequest>,
) -> Option<PlanResult> {
    let cancel_flag = Arc::new(AtomicBool::new(false));

    gdplanningai_rust::logger::init_log_channel();
    gdplanningai_rust::logger::set_log_level(gdplanningai_rust::logger::LogLevel::Debug);

    let (engine_tx, engine_rx) = mpsc::channel::<PlannerCallback>();
    let non_wildcard_actions = build_non_wildcard_actions(&actions);
    let provision_index = build_provision_index(&actions);

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
        provision_index,
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
                thread::sleep(Duration::from_millis(1));
            }
        }
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

// ── Tests ──────────────────────────────────────────────────────────

#[test]
fn requirement_provision_chains_pickup_then_eat() {
    // Agent is hungry with no held item.
    // Pickup provides held_item binding; Eat requires it.
    // Planner must chain Pickup -> Eat.
    let agent = create_test_agent(vec![("hunger", VariantSnapshot::Int(80))]);
    let world = create_test_world(
        vec![],
        vec![(
            "food_01",
            create_sim_object("food_01", vec!["food"], vec![]),
        )],
    );

    let actions = vec![
        ActionSpec {
            name: "pickup".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: None, // identity effect
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![ProvisionSpec::Binding {
                binding_name: "held_item".to_string(),
                value: VariantSnapshot::ObjectRef(101),
            }],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "eat".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(2), // reduces hunger
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![RequirementSpec::BindingExists {
                binding_name: "held_item".to_string(),
            }],
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

    let (req_tx, handle) = spawn_action_aware_responder(5.0, 2, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 2);
    assert_eq!(plan.action_chain[0], 0i64); // pickup
    assert_eq!(plan.action_chain[1], 1i64); // eat
}

#[test]
fn wildcard_fact_provision_binds_concrete_value() {
    // go_to has a FactWildcard provision at_target.
    // interact requires at_target(obj_01).
    // Verify the bound value appears in PlanResult::action_bindings.
    let agent = create_test_agent(vec![("goal_met", VariantSnapshot::Bool(false))]);
    let world = create_test_world(
        vec![],
        vec![("obj_01", create_sim_object("obj_01", vec![], vec![]))],
    );

    let actions = vec![
        ActionSpec {
            name: "go_to".to_string(),
            cost_callable_id: None,
            effect_callable_id: Some(1),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![ProvisionSpec::FactWildcard {
                fact_name: "at_target".to_string(),
            }],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "interact".to_string(),
            cost_callable_id: None,
            effect_callable_id: Some(1),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![RequirementSpec::Fact {
                fact_name: "at_target".to_string(),
                args: vec![VariantSnapshot::ObjectRef(100)],
            }],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "reach_and_use".to_string(),
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

    let result = run_planner(agent, world, actions, goals, 10, req_tx);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 2);
    // go_to should have been prepended before interact
    assert_eq!(plan.action_chain[0], 0i64);
    assert_eq!(plan.action_chain[1], 1i64);

    // Verify bindings contain the concrete object ID for both provider and consumer
    let at_target_bindings: Vec<_> = plan
        .action_bindings
        .iter()
        .filter(|(_, name, _)| name == "at_target")
        .collect();
    assert!(
        at_target_bindings.len() >= 2,
        "expected at least two at_target bindings, got {:?}",
        at_target_bindings
    );
    for (_, _, vals) in &at_target_bindings {
        assert_eq!(vals, &vec![VariantSnapshot::ObjectRef(100)]);
    }
}

#[test]
fn binding_injection_available_during_forward_validation() {
    // go_to provides at_target wildcard.
    // use_object has a custom precondition that checks the injected binding.
    // If bindings are not injected correctly, the custom precondition fails and planning fails.
    let agent = create_test_agent(vec![("done", VariantSnapshot::Bool(false))]);
    let world = create_test_world(
        vec![],
        vec![("obj_01", create_sim_object("obj_01", vec![], vec![]))],
    );

    let actions = vec![
        ActionSpec {
            name: "go_to".to_string(),
            cost_callable_id: None,
            effect_callable_id: Some(1),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![ProvisionSpec::FactWildcard {
                fact_name: "at_target".to_string(),
            }],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "use_object".to_string(),
            cost_callable_id: None,
            effect_callable_id: Some(1),
            preconditions: vec![PreconditionSpec::Custom {
                callable_id: 99,
                dependent_object_ids: vec![],
            }],
            validity_checks: vec![],
            requirements: vec![RequirementSpec::Fact {
                fact_name: "at_target".to_string(),
                args: vec![VariantSnapshot::ObjectRef(100)],
            }],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
    ];

    let goals = vec![GoalSpec {
        name: "use".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "done".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        original_index: 0,
    }];

    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    agent
                        .properties
                        .insert("done".to_string(), VariantSnapshot::Bool(true));
                    CallbackResponse::UpdatedSnapshots(agent, world)
                }
                CallbackKind::EvalCustomPrecond { .. } => {
                    // Return true only if the at_target binding contains obj_01
                    let has_correct = req.bindings.iter().any(|(name, vals)| {
                        name == "at_target" && vals.contains(&VariantSnapshot::ObjectRef(100))
                    });
                    CallbackResponse::Bool(has_correct)
                }
                _ => CallbackResponse::Float(1.0),
            };
            let _ = req.response_tx.send(PlannerCallback {
                request_id: req.request_id,
                response,
            });
        }
    });

    let result = run_planner(agent, world, actions, goals, 10, req_tx);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 2);
    // go_to must be before use_object for the binding to be available
    assert_eq!(plan.action_chain[0], 0i64);
    assert_eq!(plan.action_chain[1], 1i64);
}

#[test]
fn custom_precondition_false_prunes_branch() {
    // cursed_action has a custom precondition that returns false.
    // safe_action has no precondition and also satisfies the goal.
    // Planner should prune the cursed branch and return the safe plan.
    let agent = create_test_agent(vec![("hunger", VariantSnapshot::Int(80))]);
    let world = create_test_world(vec![], vec![]);

    let actions = vec![
        ActionSpec {
            name: "cursed_action".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(0),
            preconditions: vec![PreconditionSpec::Custom {
                callable_id: 77,
                dependent_object_ids: vec![],
            }],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "safe_action".to_string(),
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

    let (req_tx, handle) = spawn_selective_callback_responder(5.0, 60, 77);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    // Should have found safe_action, not cursed_action
    assert_eq!(plan.action_chain.len(), 1);
    assert_eq!(plan.action_chain[0], 1i64); // safe_action index
}

#[test]
fn binding_in_set_respects_world_group() {
    // Agent holds a tool (not food).
    // eat requires held_item in the "food" group.
    // pickup_food provides held_item bound to a food object.
    // Planner must select pickup_food -> eat.
    let agent = create_test_agent(vec![
        ("hunger", VariantSnapshot::Int(80)),
        ("held_item", VariantSnapshot::ObjectRef(102)),
    ]);
    let world = create_test_world(
        vec![],
        vec![
            // UIDs must match the ObjectRef integer values as strings
            ("101", create_sim_object("101", vec!["food"], vec![])),
            ("102", create_sim_object("102", vec!["tool"], vec![])),
        ],
    );

    let actions = vec![
        ActionSpec {
            name: "pickup_food".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: None, // identity effect
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![ProvisionSpec::Binding {
                binding_name: "held_item".to_string(),
                value: VariantSnapshot::ObjectRef(101),
            }],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "eat".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(2), // reduces hunger
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![RequirementSpec::BindingInSet {
                binding_name: "held_item".to_string(),
                set_name: "food".to_string(),
            }],
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

    let (req_tx, handle) = spawn_action_aware_responder(5.0, 2, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("should produce a plan");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 2);
    assert_eq!(plan.action_chain[0], 0i64); // pickup_food
    assert_eq!(plan.action_chain[1], 1i64); // eat
}

#[test]
fn action_cannot_satisfy_its_own_precondition() {
    // An action whose effect happens to satisfy its own precondition
    // must still require that precondition from the initial state or a
    // predecessor. The post-action state must NOT count.
    let agent = create_test_agent(vec![("health", VariantSnapshot::Int(0))]);
    let world = create_test_world(vec![], vec![]);

    let actions = vec![ActionSpec {
        name: "heal".to_string(),
        cost_callable_id: None,
        effect_callable_id: Some(1),
        preconditions: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(0)),
        }],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    }];

    let goals = vec![GoalSpec {
        name: "recover".to_string(),
        reward: 10.0,
        desired_state: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "health".to_string(),
            value: Some(VariantSnapshot::Int(5)),
        }],
        original_index: 0,
    }];

    let (req_tx, req_rx) = mpsc::channel::<CallbackRequest>();
    thread::spawn(move || {
        for req in req_rx {
            let response = match req.kind {
                CallbackKind::ApplyEffect {
                    mut agent, world, ..
                } => {
                    agent
                        .properties
                        .insert("health".to_string(), VariantSnapshot::Int(10));
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

    let result = run_planner(agent, world, actions, goals, 10, req_tx);

    let plan = result.expect("planner should complete");
    // The action's own effect must NOT count as satisfying its precondition.
    // The precondition must be met before the action runs; initial health=0
    // does not satisfy health>0, so the plan should fail.
    assert!(
        !plan.success,
        "should NOT produce a successful plan when the action's precondition is not met by the initial state"
    );
}

#[test]
fn binding_in_set_rejected_during_forward_validation() {
    // BindingInSet requirements must validate group membership during
    // forward validation, not just match binding names.
    // World has sword (group "weapon") and apple (group "food").
    // pickup_sword provides held_item=sword.
    // eat requires held_item in the "food" group.
    // The planner should reject the sword as satisfying the food requirement.
    let agent = create_test_agent(vec![("hunger", VariantSnapshot::Int(80))]);
    let world = create_test_world(
        vec![],
        vec![
            ("101", create_sim_object("101", vec!["food"], vec![])),
            ("102", create_sim_object("102", vec!["weapon"], vec![])),
        ],
    );

    let actions = vec![
        ActionSpec {
            name: "pickup_sword".to_string(),
            cost_callable_id: None,
            effect_callable_id: None,
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![],
            provisions: vec![ProvisionSpec::Binding {
                binding_name: "held_item".to_string(),
                value: VariantSnapshot::ObjectRef(102),
            }],
            dependent_object_ids: vec![],
        },
        ActionSpec {
            name: "eat".to_string(),
            cost_callable_id: Some(0),
            effect_callable_id: Some(2),
            preconditions: vec![],
            validity_checks: vec![],
            requirements: vec![RequirementSpec::BindingInSet {
                binding_name: "held_item".to_string(),
                set_name: "food".to_string(),
            }],
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

    let (req_tx, handle) = spawn_action_aware_responder(5.0, 2, 60);
    let result = run_planner(agent, world, actions, goals, 10, req_tx);
    drop(handle);

    let plan = result.expect("planner should complete");
    // Forward validation must enforce the group-membership constraint.
    // The sword is in group "weapon", not "food", so it must be rejected
    // as satisfying the food requirement.
    assert!(
        !plan.success,
        "should NOT produce a successful plan when the only held_item provider is not in the required group"
    );
}
