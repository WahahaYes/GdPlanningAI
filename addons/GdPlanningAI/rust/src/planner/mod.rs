pub mod types;
pub mod expander;
pub mod controller;
pub mod simulation;
pub mod heuristic;
pub mod engine;

pub use engine::PlannerEngine;
pub use types::{PlanBranch, ActionCandidate};
pub use expander::SearchContext;

use crate::plan_types::*;
use crate::snapshot::BlackboardSnapshot;
use crate::plan_tree::PlanResult;
use crate::requirement::ProvisionSpec;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::Sender;

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
    let ctx = SearchContext {
        actions: &actions,
        initial_agent: &agent,
        initial_world: &world,
        initial_provisions: &initial_provisions,
        request_tx: &request_tx,
    };

    let mut engine = PlannerEngine::new(&ctx, max_recursion, cancel_flag);
    engine.plan(&goals)
}
