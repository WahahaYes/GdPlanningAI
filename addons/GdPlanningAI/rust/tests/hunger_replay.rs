//! Headless Hunger replay: feeds one captured live Hunger submit to the engine with a
//! scripted host that replays the recorded answers, and requires the same plan
//! (action chain + cost) 20 runs in a row. Branch counts may spread: thread timing
//! shapes how much redundant work the search does, not what it finds.
//!
//! Capture: run the large campfire scene with `GDPAI_DUMP_JOB0=<dir>` set; the first
//! submit with a Hunger goal writes `<dir>/job0.json` and `<dir>/trace.jsonl`. Replay:
//! `GDPAI_DUMP_JOB0=<dir> cargo test --test hunger_replay`. Skips (ok) when unset.

mod common;

use gdplanningai_rust::logger;
use gdplanningai_rust::plan_types::{
    CallbackRequest, CallbackResponse, GoalSpec, PlannerRunResult,
};
use gdplanningai_rust::planner::{EngineConfig, EngineInputs, PlannerEngine};
use gdplanningai_rust::scheduler::{Job0Inputs, TraceLine};
use std::sync::Arc;
use std::sync::atomic::AtomicU64;
use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

/// Trace tag for each callback kind, matching the capture site.
fn kind_tag(req: &CallbackRequest) -> &'static str {
    match &req.kind {
        gdplanningai_rust::plan_types::CallbackKind::GetCost { .. } => "cost",
        gdplanningai_rust::plan_types::CallbackKind::ApplyEffect { .. } => "effect",
        gdplanningai_rust::plan_types::CallbackKind::EvalCustomPrecond { .. } => "precond",
    }
}

/// Map-based host: answers every request from the values the live run saw, keyed by
/// (kind, registry id, bindings) — independent of request order, so thread timing cannot
/// shape the replay. A request the live run never made fails loudly: that is the signal.
fn spawn_map_host(
    costs: std::collections::HashMap<String, f64>,
    preconds: std::collections::HashMap<String, bool>,
) -> Sender<CallbackRequest> {
    common::spawn_responder(move |req| {
        let key = request_key(&req);
        match &req.kind {
            gdplanningai_rust::plan_types::CallbackKind::GetCost { .. } => CallbackResponse::Float(
                *costs
                    .get(&key)
                    .unwrap_or_else(|| panic!("cost request never seen live: {key}")),
            ),
            gdplanningai_rust::plan_types::CallbackKind::EvalCustomPrecond { .. } => {
                CallbackResponse::Bool(
                    *preconds
                        .get(&key)
                        .unwrap_or_else(|| panic!("precond request never seen live: {key}")),
                )
            }
            gdplanningai_rust::plan_types::CallbackKind::ApplyEffect { .. } => {
                panic!("effect fallback in replay: none occurred live: {key}")
            }
        }
    })
}

/// Canonical JSON: object keys sorted, so `HashMap` iteration order (per-process random)
/// cannot perturb request keys. The capture and replay sides both canonicalize.
fn canonical(value: serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .map(|(k, v)| (k, canonical(v)))
                .collect::<std::collections::BTreeMap<_, _>>()
                .into_iter()
                .collect(),
        ),
        serde_json::Value::Array(items) => {
            serde_json::Value::Array(items.into_iter().map(canonical).collect())
        }
        other => other,
    }
}

/// Canonical key for one request: kind tag, registry id, evaluation state and bindings.
fn request_key(req: &CallbackRequest) -> String {
    let (agent, world) = match &req.kind {
        gdplanningai_rust::plan_types::CallbackKind::GetCost { agent, world }
        | gdplanningai_rust::plan_types::CallbackKind::ApplyEffect { agent, world }
        | gdplanningai_rust::plan_types::CallbackKind::EvalCustomPrecond { agent, world } => {
            (agent, world)
        }
    };
    format!(
        "{}:{}:{}:{}:{}",
        kind_tag(req),
        req.callable_id,
        canonical(serde_json::to_value(&**agent).expect("agent serializes")),
        canonical(serde_json::to_value(&**world).expect("world serializes")),
        canonical(serde_json::to_value(&req.bindings).expect("bindings serialize"))
    )
}

/// Indexes the trace answers by request key. Duplicate keys must agree.
fn index_trace(
    trace: &[TraceLine],
) -> (
    std::collections::HashMap<String, f64>,
    std::collections::HashMap<String, bool>,
) {
    let mut costs = std::collections::HashMap::new();
    let mut preconds = std::collections::HashMap::new();
    for line in trace {
        let key = format!(
            "{}:{}:{}:{}:{}",
            line.kind,
            line.callable_id,
            canonical(serde_json::to_value(&line.agent).expect("agent serializes")),
            canonical(serde_json::to_value(&line.world).expect("world serializes")),
            canonical(serde_json::to_value(&line.bindings).expect("bindings serialize"))
        );
        match &line.response {
            gdplanningai_rust::scheduler::TraceResponse::Float(f) => {
                let prev = costs.insert(key, *f);
                assert!(prev.is_none_or(|p| p == *f), "cost answers must agree");
            }
            gdplanningai_rust::scheduler::TraceResponse::Bool(b) => {
                let prev = preconds.insert(key, *b);
                assert!(prev.is_none_or(|p| p == *b), "precond answers must agree");
            }
            gdplanningai_rust::scheduler::TraceResponse::Snapshots { .. } => {
                panic!("effect fallback occurred live; the replay host cannot serve it")
            }
        }
    }
    (costs, preconds)
}

/// Steps `engine` to completion without Godot; panics after 120 s.
fn run_to_completion(
    engine: &mut PlannerEngine,
    goals: &[GoalSpec],
) -> Option<gdplanningai_rust::plan_tree::PlanResult> {
    let start = Instant::now();
    loop {
        assert!(
            start.elapsed() < Duration::from_secs(120),
            "timed out waiting for plan completion"
        );
        match engine.plan(goals) {
            PlannerRunResult::Complete(res) => return res,
            // Generous sleep: answers always land before the next step, so thread timing
            // cannot shape the replay the way it shapes a live run.
            PlannerRunResult::Pending(_) => std::thread::sleep(Duration::from_millis(10)),
        }
    }
}

/// `Branches: N` from the one-line search summary.
fn branch_count(engine: &PlannerEngine) -> usize {
    let summary = engine.debug_tree().summary();
    let marker = "Branches: ";
    let start = summary.find(marker).expect("summary reports branch counts") + marker.len();
    summary[start..]
        .split_whitespace()
        .next()
        .expect("count follows the marker")
        .parse()
        .expect("count is a number")
}

#[test]
fn hunger_job0_replays_identically_20x() {
    let Ok(dir) = std::env::var("GDPAI_DUMP_JOB0") else {
        eprintln!("GDPAI_DUMP_JOB0 unset: no captured submit, skipping replay");
        return;
    };
    logger::init_log_channel();
    logger::set_log_level(logger::LogLevel::Debug);

    let inputs_text =
        std::fs::read_to_string(format!("{dir}/job0.json")).expect("captured inputs readable");
    let job: Job0Inputs = serde_json::from_str(&inputs_text).expect("captured inputs parse");
    let trace_text =
        std::fs::read_to_string(format!("{dir}/trace.jsonl")).expect("captured trace readable");
    let trace: Vec<TraceLine> = trace_text
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()
        .expect("captured trace parses");
    assert!(!trace.is_empty(), "the trace must hold at least one answer");

    let (costs, preconds) = index_trace(&trace);

    let mut counts = Vec::with_capacity(20);
    let mut chains: Vec<Vec<i64>> = Vec::with_capacity(20);
    let mut run_costs = Vec::with_capacity(20);
    for run in 0..20 {
        let req_tx = spawn_map_host(costs.clone(), preconds.clone());
        // Time-slicing yields on wall-clock reads, which would let thread timing shape
        // the replay; disable it so the search is a pure function of inputs + answers.
        let config = EngineConfig {
            time_slice_ms: 0,
            ..EngineConfig::default()
        };
        let mut engine = PlannerEngine::new(
            EngineInputs {
                actions: job.actions.clone(),
                agent: job.agent.clone(),
                world: job.world.clone(),
                initial_provisions: job.initial_provisions.clone(),
                request_tx: req_tx,
                cancel_flag: Arc::default(),
                effect_ir_hits: Arc::new(AtomicU64::new(0)),
                effect_fallback_budget: EngineConfig::default().max_effect_fallbacks,
                effect_fallback_trips: Arc::new(AtomicU64::new(0)),
            },
            config,
        );
        let plan = run_to_completion(&mut engine, &job.goals);
        let plan = plan.expect("run {run}: the search must finish");
        assert!(
            plan.success,
            "run {run}: the captured job found a plan live"
        );
        counts.push(branch_count(&engine));
        eprintln!(
            "run {run}: chain={:?} cost={:.1} branches={}",
            plan.action_chain, plan.total_cost, counts[run]
        );
        run_costs.push(plan.total_cost);
        chains.push(plan.action_chain);
    }
    assert!(
        chains.iter().all(|c| c == &chains[0]),
        "all 20 replays must find the same plan, got {chains:?}"
    );
    assert!(
        run_costs.iter().all(|c| *c == run_costs[0]),
        "all 20 replays must agree on cost, got {run_costs:?}"
    );
    assert!(
        counts.iter().all(|&c| c == counts[0]),
        "all 20 replays must agree, got {counts:?}"
    );
    eprintln!(
        "hunger replay: 20x identical, branches={} actions={:?}",
        counts[0], chains[0]
    );
}
