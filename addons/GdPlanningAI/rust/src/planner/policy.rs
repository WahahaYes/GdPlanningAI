use super::controller::SearchController;
use super::stats::SearchStats;

pub trait TerminationPolicy {
    fn on_search_start(&mut self, _max_depth: usize) {}
    fn should_terminate(&self, stats: &SearchStats, controller: &dyn SearchController) -> bool;
    fn on_valid_plan_found(&mut self, _cost: f64, _stats: &SearchStats) {}
}

pub struct FirstValidPolicy;

impl FirstValidPolicy {
    pub fn new() -> Self {
        Self
    }
}

impl TerminationPolicy for FirstValidPolicy {
    fn should_terminate(&self, stats: &SearchStats, _controller: &dyn SearchController) -> bool {
        stats.valid_plans_found > 0
    }
}

pub struct ExhaustivePolicy;

impl ExhaustivePolicy {
    pub fn new() -> Self {
        Self
    }
}

impl TerminationPolicy for ExhaustivePolicy {
    fn should_terminate(&self, _stats: &SearchStats, controller: &dyn SearchController) -> bool {
        controller.frontier_size() == 0
    }
}
