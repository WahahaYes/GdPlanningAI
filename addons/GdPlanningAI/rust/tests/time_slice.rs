//! Regression test for wall-clock time-slicing preserving plan results.
//!
//! Verifies that planning with time-slicing disabled (`time_slice_ms = 0`)
//! and enabled (1ms slice) yields the same final plan. The engine reads
//! [`fake_clock`], which advances 1ms per read, so a slice expires at the
//! first check however fast the build runs the search.

mod common;
use common::{inputs, reduce_hunger, run_counting_yields, spawn_responder};

use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerRunResult,
    PreconditionSpec,
};
use gdplanningai_rust::planner::{EngineConfig, EngineInputs, PlannerEngine};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::LazyLock;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use std::time::{Duration, Instant};

static CLOCK_BASE: LazyLock<Instant> = LazyLock::new(Instant::now);
static CLOCK_TICKS: AtomicU64 = AtomicU64::new(0);

/// A clock that moves 1ms forward on every read.
fn fake_clock() -> Instant {
    *CLOCK_BASE + Duration::from_millis(CLOCK_TICKS.fetch_add(1, Ordering::Relaxed))
}

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

const EAT_EFFECT: usize = 0;
const GET_FOOD_EFFECT: usize = 1;

fn make_actions_with_count(count: usize) -> Vec<ActionSpec> {
    // One eat that needs food, plus `count` identical ways to get food, so
    // every plan is get_food_i then eat. Verification of each branch waits on
    // a callback for eat (discovery only covers step 0), which forces >128
    // queue pops so the wall-clock slice gate (checked every 128 pops) is
    // exercised.
    let eat = ActionSpec {
        name: "eat".to_string(),
        cost_callable_id: Some(0),
        effect_callable_id: Some(EAT_EFFECT),
        preconditions: vec![PreconditionSpec::Builtin {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::Equal,
            property_name: "has_food".to_string(),
            value: Some(VariantSnapshot::Bool(true)),
        }],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    };
    let get_food = (0..count).map(|i| ActionSpec {
        name: format!("get_food_{}", i),
        cost_callable_id: Some(0),
        effect_callable_id: Some(GET_FOOD_EFFECT),
        preconditions: vec![],
        validity_checks: vec![],
        requirements: vec![],
        provisions: vec![],
        dependent_object_ids: vec![],
    });
    std::iter::once(eat).chain(get_food).collect()
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
    spawn_responder(|req| match req.kind {
        CallbackKind::GetCost { .. } => CallbackResponse::Float(5.0),
        CallbackKind::ApplyEffect { mut agent, world } => {
            if req.callable_id == GET_FOOD_EFFECT {
                agent
                    .properties
                    .insert("has_food".to_string(), VariantSnapshot::Bool(true));
            } else {
                reduce_hunger(&mut agent, 60);
            }
            CallbackResponse::UpdatedSnapshots(agent, world)
        }
        _ => CallbackResponse::Float(1.0),
    })
}

fn run_to_completion(time_slice_ms: u64, action_count: usize) -> (Option<PlanResult>, usize) {
    let mut engine = PlannerEngine::new(
        inputs(
            make_actions_with_count(action_count),
            make_agent(),
            make_world(),
            spawn_callback_responder(),
        ),
        EngineConfig {
            max_depth: 10,
            time_slice_ms,
            clock: fake_clock,
            ..EngineConfig::default()
        },
    );
    run_counting_yields(&mut engine, &make_goals())
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
    assert_eq!(disabled.action_chain.len(), 2);
    assert_eq!(disabled.total_cost, 10.0);
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
    // pops) must never fire, even though every check would find the slice spent.
    let (plan, pending_zero_count) = run_to_completion(1, 5);

    let plan = plan.expect("small fan-out should produce a plan");
    assert!(plan.success, "small fan-out should succeed");
    assert_eq!(plan.action_chain.len(), 2);
    assert_eq!(plan.total_cost, 10.0);
    assert_eq!(
        pending_zero_count, 0,
        "fewer than 128 pops must never yield Pending(0) via timer"
    );
}

#[test]
fn cancel_returns_complete_immediately() {
    // Cancel flag set before planning must win over time-slice yield:
    // plan() should return Complete(None), never Pending(0).
    let goals = make_goals();
    let mut engine = PlannerEngine::new(
        EngineInputs {
            cancel_flag: Arc::new(AtomicBool::new(true)),
            ..inputs(
                make_actions_with_count(300),
                make_agent(),
                make_world(),
                spawn_callback_responder(),
            )
        },
        EngineConfig {
            max_depth: 10,
            time_slice_ms: 1,
            ..EngineConfig::default()
        },
    );

    match engine.plan(&goals) {
        PlannerRunResult::Complete(None) => {}
        other => panic!("cancel should return Complete(None), got {:?}", other),
    }
}
