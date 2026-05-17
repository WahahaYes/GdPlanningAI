//! Search statistics tracking.
//!
//! Tracks metrics such as branches expanded, pruned, and depth reached during
//! the search process.

use std::time::Instant;

/// Collection of search performance and outcome metrics.
#[derive(Debug)]
pub struct SearchStats {
    pub branches_expanded: usize,
    pub branches_pruned: usize,
    pub max_depth_reached: usize,
    pub valid_plans_found: usize,
    pub best_cost: f64,
    start_time: Instant,
}

impl SearchStats {
    /// Create a new stats instance starting from now.
    pub fn new() -> Self {
        Self {
            branches_expanded: 0,
            branches_pruned: 0,
            max_depth_reached: 0,
            valid_plans_found: 0,
            best_cost: f64::INFINITY,
            start_time: Instant::now(),
        }
    }

    /// Returns elapsed time in milliseconds since creation.
    pub fn elapsed_ms(&self) -> u64 {
        self.start_time.elapsed().as_millis() as u64
    }
}

impl Default for SearchStats {
    fn default() -> Self {
        Self::new()
    }
}
