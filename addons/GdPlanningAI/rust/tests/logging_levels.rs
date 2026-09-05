//! Integration tests for planner log-level behavior.
//!
//! These tests prove the logging slim-down contract:
//! - At [`LogLevel::Info`] (the default) a plan still completes, no full
//!   search tree is recorded, but [`TreeDump::summary`] reports real branch
//!   counts for the one-line `RESULT` log.
//! - At [`LogLevel::Debug`] the full tree is recorded again and
//!   [`TreeDump::format`] produces the `PLANNER SEARCH TREE` dump.
//! - [`LogLevel::Trace`] (opt-in level 4) re-enables per-iteration detail
//!   (`find_candidates`, simulation payloads) that [`LogLevel::Debug`]
//!   suppresses.

use gdplanningai_rust::debug_tree::TreeDump;
use gdplanningai_rust::logger::{LogLevel, get_log_level, init_log_channel, set_log_level};
use gdplanningai_rust::plan_tree::PlanResult;
use gdplanningai_rust::plan_types::{
    ActionSpec, CallbackKind, CallbackRequest, CallbackResponse, GoalSpec, PlannerCallback,
    PlannerRunResult, PreconditionSpec,
};
use gdplanningai_rust::planner::{
    DijkstraHeuristic, PlannerEngine, SearchContext, TerminationStrategy,
};
use gdplanningai_rust::precondition::{PreconditionOp, PreconditionTarget};
use gdplanningai_rust::scheduler::StallNoticeThrottle;
use gdplanningai_rust::snapshot::{BlackboardSnapshot, VariantSnapshot};
use gdplanningai_rust::{log_debug, log_info, log_trace};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

/// Serializes tests that mutate the process-global log level, since cargo
/// runs tests in one binary on multiple threads sharing that state.
static LEVEL_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

// ── Helpers ──────────────────────────────────────────────────────────

fn make_agent(hunger: i64) -> BlackboardSnapshot {
    let mut props = HashMap::new();
    props.insert("hunger".to_string(), VariantSnapshot::Int(hunger));
    BlackboardSnapshot {
        properties: props,
        objects: HashMap::new(),
    }
}

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
            let _ = req.response_tx.send(PlannerCallback {
                request_id: req.request_id,
                response,
            });
        }
    });
    (req_tx, handle)
}

struct PlanOutcome {
    plan: Option<PlanResult>,
    tree_enabled: bool,
    summary: String,
    tree_dump: String,
}

/// Runs the single-action eat plan at `level` and captures tree state.
/// The level is set immediately before engine construction (which is when
/// [`TreeDump`] reads it), and restored to the previous value afterwards.
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

    let (req_tx, handle) = spawn_callback_responder(5.0, 60);
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
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let mut engine = PlannerEngine::new(ctx, 10, cancel_flag)
        .with_heuristic(Box::new(DijkstraHeuristic))
        .with_termination_strategy(TerminationStrategy::BestCost);
    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start = Instant::now();
    let plan = loop {
        if start.elapsed() > Duration::from_secs(5) {
            panic!("run_eat_plan: timed out waiting for plan completion");
        }
        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => break res,
            PlannerRunResult::Pending(_) => thread::sleep(Duration::from_millis(1)),
        }
    };
    drop(handle);

    PlanOutcome {
        plan,
        tree_enabled: engine.tree.is_enabled(),
        summary: engine.tree.summary(),
        tree_dump: engine.tree.format(),
    }
}

// ── Tests ────────────────────────────────────────────────────────────

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

#[test]
fn debug_reenables_full_tree_detail() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    let outcome = run_eat_plan_at_level(LogLevel::Debug);

    let plan = outcome.plan.expect("plan should complete at Debug");
    assert!(plan.success);

    assert!(outcome.tree_enabled, "tree recording must resume at Debug");
    assert!(
        outcome.tree_dump.contains("PLANNER SEARCH TREE"),
        "Debug must re-enable the full tree dump"
    );
    assert!(outcome.summary.contains("Branches: "));
}

#[test]
fn trace_level_reenables_per_iteration_detail() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    assert_eq!(LogLevel::from_u8(4), LogLevel::Trace);
    assert!(LogLevel::Trace.allows(LogLevel::Trace));
    assert!(!LogLevel::Debug.allows(LogLevel::Trace));
    assert!(!LogLevel::Info.allows(LogLevel::Trace));

    // Macro smoke test: emitting at each level must not panic, whether or
    // not the channel drains (process_logs is a no-op under cfg(test)).
    init_log_channel();
    let previous = get_log_level();
    set_log_level(LogLevel::Trace);
    log_trace!("trace smoke");
    log_debug!("debug smoke");
    log_info!("info smoke");
    set_log_level(LogLevel::Info);
    log_trace!("suppressed trace smoke");
    set_log_level(previous);
}

#[test]
fn stall_notice_logs_once_per_pending_id() {
    let mut throttle = StallNoticeThrottle::default();
    assert!(throttle.should_log(3));
    assert!(!throttle.should_log(3));
    assert!(throttle.should_log(4));
    throttle.reset();
    assert!(throttle.should_log(4));
}

#[test]
fn disabled_tree_still_counts_branches_for_summary() {
    let _guard = LEVEL_LOCK.lock().unwrap();
    init_log_channel();
    let previous = get_log_level();
    set_log_level(LogLevel::Info);

    let mut dump = TreeDump::new();
    assert!(!dump.is_enabled());
    dump.begin_goal("g", 1.0, &[]);
    let root = dump.add_root(&[], &[]);
    dump.add_child(
        root,
        gdplanningai_rust::debug_tree::ChildNodeConfig {
            action_name: "a",
            estimated_cost: 1.0,
            accumulated_cost: 1.0,
            open_pre: &[],
            open_req: &[],
            satisfied_pre: &[],
            satisfied_req: &[],
        },
    );
    assert!(dump.format().is_empty());
    assert!(
        dump.summary().contains("Branches: 2"),
        "got: {}",
        dump.summary()
    );

    set_log_level(previous);
}
