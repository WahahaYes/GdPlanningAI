//! Termination policies for the planning process.
//!
//! Provides the [`TerminationPolicy`] trait and implementations like [`FirstValidPolicy`]
//! and [`ExhaustivePolicy`] to control when search stops.

use super::controller::SearchController;
use super::stats::SearchStats;

/// Strategy for determining when to stop searching.
pub trait TerminationPolicy {
    /// Called when search begins for a goal.
    fn on_search_start(&mut self, _max_depth: usize) {}
    /// Returns true if search should terminate.
    fn should_terminate(&self, stats: &SearchStats, controller: &dyn SearchController) -> bool;
    /// Called when a valid plan is found.
    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

/// Stop as soon as any valid plan is found.
pub struct FirstValidPolicy;

impl FirstValidPolicy {
    /// Create a new instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for FirstValidPolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminationPolicy for FirstValidPolicy {
    fn should_terminate(&self, stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        stats.valid_plans_found > 0
    }
}

/// Explore all reachable branches to find the best plan.
pub struct ExhaustivePolicy;

impl ExhaustivePolicy {
    /// Create a new instance.
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExhaustivePolicy {
    fn default() -> Self {
        Self::new()
    }
}

impl TerminationPolicy for ExhaustivePolicy {
    fn should_terminate(&self, _stats: &SearchStats, controller: &dyn SearchController) -> bool {
        controller.frontier_size() == 0
    }
}
