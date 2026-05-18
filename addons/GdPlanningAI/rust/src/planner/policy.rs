//! Termination policies for the planning process.
//!
//! Provides the [`TerminationPolicy`] trait and implementations like [`FirstValidPolicy`]
//! and [`ExhaustivePolicy`] to control when search stops.

use crate::planner::controller::SearchController;
use crate::planner::stats::SearchStats;

/// Strategy for determining when to stop searching.
pub trait TerminationPolicy {
    /// Called once when the search for a specific goal begins.
    fn on_search_start(&mut self, max_depth: usize);

    /// Returns true if the search should terminate.
    fn should_terminate(&self, stats: &SearchStats, controller: &dyn SearchController) -> bool;

    /// Called when a valid plan is found.
    fn on_valid_plan_found(&mut self, cost: f64, stats: &SearchStats);
}

/// Terminates after the first valid plan is found.
pub struct FirstValidPolicy {}

impl FirstValidPolicy {
    pub fn new() -> Self {
        Self {}
    }
}

impl TerminationPolicy for FirstValidPolicy {
    fn on_search_start(&mut self, _max_depth: usize) {}

    fn should_terminate(&self, stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        stats.valid_plans_found > 0
    }

    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

/// Never terminates early; explores the entire reachable search space.
pub struct ExhaustivePolicy {}

impl ExhaustivePolicy {
    pub fn new() -> Self {
        Self {}
    }
}

impl TerminationPolicy for ExhaustivePolicy {
    fn on_search_start(&mut self, _max_depth: usize) {}

    fn should_terminate(&self, _stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        false
    }

    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

/// Terminates based on time or branch count budgets.
pub struct BudgetPolicy {
    max_time_ms: u64,
    max_branches: usize,
}

impl BudgetPolicy {
    pub fn new(max_time_ms: u64, max_branches: usize) -> Self {
        Self {
            max_time_ms,
            max_branches,
        }
    }
}

impl TerminationPolicy for BudgetPolicy {
    fn on_search_start(&mut self, _max_depth: usize) {}

    fn should_terminate(&self, stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        if self.max_time_ms > 0 && stats.elapsed_ms() >= self.max_time_ms {
            return true;
        }
        if self.max_branches > 0 && stats.branches_expanded >= self.max_branches {
            return true;
        }
        false
    }

    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

/// Terminates if time/branch budget exceeded AND at least one plan found.
/// If no plan found, continues until budget exceeded.
pub struct BestWithinBudgetPolicy {
    max_time_ms: u64,
    max_branches: usize,
}

impl BestWithinBudgetPolicy {
    pub fn new(max_time_ms: u64, max_branches: usize) -> Self {
        Self {
            max_time_ms,
            max_branches,
        }
    }
}

impl TerminationPolicy for BestWithinBudgetPolicy {
    fn on_search_start(&mut self, _max_depth: usize) {}

    fn should_terminate(&self, stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        let budget_exceeded = (self.max_time_ms > 0 && stats.elapsed_ms() >= self.max_time_ms)
            || (self.max_branches > 0 && stats.branches_expanded >= self.max_branches);

        if budget_exceeded {
            return true;
        }

        false
    }

    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

