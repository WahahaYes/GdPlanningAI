//! GdPlanningAI's Rust extension: a backward-chaining GOAP planner exposed to Godot.
//!
//! Flow: GDScript calls `submit_plan` on [`scheduler::GdPAIPlanScheduler`], which snapshots
//! state into Send-safe [`snapshot`] and [`plan_types`] data. A [`planner::PlannerEngine`] steps
//! on a Rayon worker and asks the main thread, via `process_callbacks`, to run GDScript costs,
//! effects and custom preconditions. See `docs/ALGORITHM.md`.

use godot::prelude::*;

#[macro_use]
pub mod logger;
pub mod debug_tree;
pub mod gdpai_blackboard;
pub mod plan_tree;
pub mod plan_types;
pub mod planner;
pub mod precondition;
pub mod requirement;
pub mod scheduler;
pub mod sim_object_proxy;
pub mod snapshot;

/// GDExtension entry point; registered through the `gdext_rust_init` symbol.
struct GdPlanningAIExt;

#[gdextension]
unsafe impl ExtensionLibrary for GdPlanningAIExt {
    /// Creates the global log channel as early as possible so that log lines
    /// emitted from planner threads have somewhere to go.
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Servers {
            crate::logger::init_log_channel();
        }
    }
}
