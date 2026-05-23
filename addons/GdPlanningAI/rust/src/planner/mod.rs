pub mod types;
pub mod simulation;
pub mod expander;
pub mod engine;

pub use engine::PlannerEngine;
pub use types::{PlanBranch, SearchContext};

use crate::plan_types::*;
use crate::snapshot::BlackboardSnapshot;
use crate::plan_tree::PlanResult;
use crate::requirement::ProvisionSpec;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;
use std::collections::HashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchAlgorithm {
    AStar,
    Dijkstra,
    DepthFirst,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminationStrategy {
    FirstComplete,
    BestCost,
}


pub fn run_plan(
    actions: Vec<ActionSpec>,
    goals: Vec<GoalSpec>,
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    initial_provisions: Vec<ProvisionSpec>,
    max_recursion: usize,
    request_tx: Sender<CallbackRequest>,
    cancel_flag: Arc<AtomicBool>,
) -> Option<PlanResult> {
    let (engine_tx, engine_rx) = std::sync::mpsc::channel::<PlannerCallback>();

    let ctx = Arc::new(SearchContext {
        actions,
        initial_agent: agent,
        initial_world: world,
        initial_provisions,
        request_tx,
        engine_response_tx: engine_tx.clone(),
        discovery_results: std::sync::Mutex::new(HashMap::new()),
        discovery_pending: std::sync::Mutex::new(HashMap::new()),
        discovery_request_map: std::sync::Mutex::new(HashMap::new()),
    });

    let mut engine = PlannerEngine::new(ctx, max_recursion, cancel_flag);
    engine.response_rx = engine_rx;
    engine.response_tx = engine_tx;

    let start_time = std::time::Instant::now();
    let timeout = std::time::Duration::from_secs(5);

    loop {
        if start_time.elapsed() > timeout {
            log_error!("run_plan: Timed out waiting for plan completion");
            return None;
        }

        match engine.plan(&goals) {
            PlannerRunResult::Complete(res) => return res,
            PlannerRunResult::Pending(_) => {
                // In synchronous mode, we expect another thread (or the test runner) 
                // to be processing requests and sending responses.
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
    }
}
