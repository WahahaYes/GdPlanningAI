//! GdPlanningAI Rust extension library.
//!
//! Provides a backward-chaining GOAP (Goal-Oriented Action Planning) engine
//! exposed to Godot 4 via GDExtension. Users interact with it entirely from
//! GDScript; no Rust knowledge is required.
//!
//! # Planning flow
//!
//! 1. GDScript calls `submit_plan` on the [`scheduler::GdPAIPlanScheduler`] with the agent and world
//!    [`gdpai_blackboard::GdPAIBlackboard`]s plus serialized actions and goals. On the main thread
//!    the scheduler converts everything into Send-safe data: [`snapshot::BlackboardSnapshot`]s,
//!    [`plan_types::ActionSpec`]s and [`plan_types::GoalSpec`]s. Every GDScript `Callable` is
//!    parked in a per-job registry and replaced by an index.
//! 2. A [`planner::PlannerEngine`] is stepped on a Rayon worker thread. It never touches Godot
//!    objects. Whenever it needs a cost, an effect or a custom precondition evaluated, it sends a
//!    [`plan_types::CallbackRequest`] to the main thread and parks the search node.
//! 3. Each frame GDScript calls `process_callbacks` on the scheduler, which runs the requested
//!    callables on the main thread, answers with a [`plan_types::PlannerCallback`], resumes the
//!    engine, and finally hands the finished [`plan_tree::PlanResult`] back to the agent.
//!
//! # Public Godot classes
//! - [`gdpai_blackboard::GdPAIBlackboard`] — key/value store for agent and world state.
//! - [`sim_object_proxy::SimObjectProxy`] — simulation snapshot of a world object.
//! - [`scheduler::GdPAIPlanScheduler`] — planning scheduler.
//!
//! # Internal types
//! Planning is performed asynchronously on a Rayon thread pool using Send-safe
//! snapshot types in [`snapshot`] and spec types in [`plan_types`] and [`requirement`].
//! The search itself lives in [`planner`]; [`debug_tree`] records it for inspection and
//! [`logger`] routes log output from planner threads to the main thread.

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
