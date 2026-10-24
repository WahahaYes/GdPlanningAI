//! `search-is-debuggable`: at [`LogLevel::Info`] plans complete and `TreeDump::summary` reports
//! branch counts without recording a tree; at [`LogLevel::Debug`] `TreeDump::format` produces
//! the full `PLANNER SEARCH TREE` dump.

mod common;
use common::{
    create_sim_object, create_test_world, hunger_answers, inputs, run_to_completion,
    spawn_responder,
};

use gdplanningai_rust::logger::{LogLevel, get_log_level, init_log_channel, set_log_level};
use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{ActionSpec, GoalSpec, PreconditionSpec};
use gdplanningai_rust::planner::{EngineConfig, PlannerEngine};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::requirement::{ProvisionSpec, RequirementSpec};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;

/// Serializes tests that mutate the process-global log level, since cargo
/// runs tests in one binary on multiple threads sharing that state.
static LEVEL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

fn make_agent(hunger: i64) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(hunger));
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

struct PlanOutcome {
    plan: Option<PlanResult>,
    tree_enabled: bool,
    summary: String,
    tree_dump: String,
}

/// Runs the eat plan at `level` and captures tree state. The higher-reward goal, hunger below 0,
/// is out of reach (one eat leaves 20), so the search tries it and falls back to the second, for
/// which eat and snack are equal alternatives.
/// The level is set immediately before engine construction (which is when
/// `TreeDump` reads it), and restored to the previous value afterwards.
fn run_eat_plan_at_level(level: LogLevel) -> PlanOutcome {
    init_log_channel();
    let previous = get_log_level();
    set_log_level(level);

    let outcome = run_eat_plan();

    set_log_level(previous);
    outcome
}

fn run_eat_plan() -> PlanOutcome {
    let agent = make_agent(80);
    let world = BlackboardSnapshot {
        properties: HashMap::new(),
        objects: HashMap::new(),
    };
    let food = |name: &str| ActionSpec {
        name: name.to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(0),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };
    let actions = vec![food("eat"), food("snack")];
    let hunger_below = |threshold: i64| {
        vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::LessThan,
            property_name: "hunger".to_string(),
            value: Some(VariantSnapshot::Int(threshold)),
        }]
    };
    let goals = vec![
        GoalSpec {
            name: "never_hungry".to_string(),
            reward: 20.0,
            desired_state: hunger_below(0),
            original_index: 0,
        },
        GoalSpec {
            name: "satisfy_hunger".to_string(),
            reward: 10.0,
            desired_state: hunger_below(30),
            original_index: 1,
        },
    ];

    let mut engine = PlannerEngine::new(
        inputs(
            actions,
            agent,
            world,
            spawn_responder(hunger_answers(5.0, 60)),
        ),
        EngineConfig {
            max_depth: 10,
            ..EngineConfig::default()
        },
    );
    let plan = run_to_completion(&mut engine, &goals);

    PlanOutcome {
        plan,
        tree_enabled: engine.debug_tree().is_enabled(),
        summary: engine.debug_tree().summary(),
        tree_dump: engine.debug_tree().format(),
    }
}

#[test]
fn info_completes_plan_with_summary_but_no_tree() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    let outcome = run_eat_plan_at_level(LogLevel::Info);

    let plan = outcome.plan.expect("plan should complete at Info");
    assert!(plan.success);
    assert_eq!(plan.action_chain.len(), 1);

    assert!(
        !outcome.tree_enabled,
        "full tree must stay off at Info to avoid recording overhead"
    );
    assert!(outcome.tree_dump.is_empty(), "no tree dump text at Info");
    assert!(
        outcome.summary.contains("Branches: "),
        "Info one-liner still reports branch counts, got: {}",
        outcome.summary
    );
    assert!(
        !outcome.summary.contains("Branches: 0"),
        "branch counter must run even when the tree is disabled, got: {}",
        outcome.summary
    );
}

/// The Debug tree shows each goal tried, its result and the actions tried for it.
#[test]
fn debug_records_each_goal_and_the_actions_tried() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    let outcome = run_eat_plan_at_level(LogLevel::Debug);

    let plan = outcome.plan.expect("plan should complete at Debug");
    assert!(plan.success);
    assert_eq!(plan.goal_index, 1);

    assert!(outcome.tree_enabled, "tree recording must resume at Debug");
    let dump = &outcome.tree_dump;
    let failed = dump.find("never_hungry").expect("first goal in the tree");
    let succeeded = dump
        .find("satisfy_hunger")
        .expect("second goal in the tree");
    assert!(failed < succeeded, "goals in the order tried:\n{dump}");
    assert!(
        dump[failed..succeeded].contains("FAILURE — no plan found"),
        "{dump}"
    );
    assert!(dump[succeeded..].contains("SUCCESS"), "{dump}");
    assert!(dump[succeeded..].contains("Try 'eat'"), "{dump}");
    assert!(dump[succeeded..].contains("Try 'snack'"), "{dump}");
    assert!(dump.contains("PLANNER SEARCH TREE"));
    assert!(outcome.summary.contains("Branches: "));
}

/// A plan rejected in verification names the failed requirement on its tree node.
#[test]
fn verification_requirement_failure_is_recorded_on_tree() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    init_log_channel();
    let previous = get_log_level();
    set_log_level(LogLevel::Debug);

    // Go To provides at_target for any object while planning, but its effect never moves the
    // agent, so replaying the plan finds the agent away from the target.
    let mut agent = make_agent(80);
    agent.objects.insert(
        "agent_location".to_string(),
        create_sim_object(
            "agent_location",
            vec!["GdPAILocationData"],
            vec![("position", VariantSnapshot::Int(0))],
        ),
    );
    let world = create_test_world(
        vec![],
        vec![(
            "100",
            create_sim_object("100", vec![], vec![("position", VariantSnapshot::Int(5))]),
        )],
    );
    let action = |name: &str| ActionSpec {
        name: name.to_string(),
        cost_callable_id: None,
        effect_callable_id: None,
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };
    let mut go_to = action("go_to");
    go_to.provisions = vec![ProvisionSpec::FactWildcard {
        fact_name: "at_target".to_string(),
    }];
    let mut eat = action("eat");
    eat.effect_callable_id = Some(0);
    eat.requirements = vec![RequirementSpec::Fact {
        fact_name: "at_target".to_string(),
        args: vec![VariantSnapshot::ObjectRef(100)],
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
    let mut engine = PlannerEngine::new(
        inputs(
            vec![go_to, eat],
            agent,
            world,
            spawn_responder(hunger_answers(5.0, 60)),
        ),
        EngineConfig::default(),
    );
    let plan = run_to_completion(&mut engine, &goals).expect("search completes");
    let dump = engine.debug_tree().format();
    set_log_level(previous);

    assert!(!plan.success, "the agent never reaches the target");
    assert!(
        dump.contains("FWD [FAIL]") && dump.contains("at_target") && dump.contains("PRUNED"),
        "failed requirement must be attributed on the tree node, got:\n{}",
        dump
    );
}
