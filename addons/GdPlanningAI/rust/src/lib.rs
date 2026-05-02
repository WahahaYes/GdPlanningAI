//! GdPlanningAI Rust extension library.
//!
//! Provides a forward-chaining GOAP (Goal-Oriented Action Planning) engine
//! exposed to Godot 4 via GDExtension. Users interact with it entirely from
//! GDScript; no Rust knowledge is required.
//!
//! # Public Godot classes
//! - [`gdpai_blackboard::GdPAIBlackboard`] — key/value store for agent and world state.
//! - [`sim_object_proxy::SimObjectProxy`] — simulation snapshot of a world object.
//! - [`scheduler::GdPAIPlanScheduler`] — background planning scheduler.
//!
//! # Internal types
//! Planning is performed asynchronously on a Rayon thread pool using Send-safe
//! snapshot types in [`background_types`].

use godot::prelude::*;

#[macro_use]
pub mod logger;
pub mod background_plan;
pub mod background_types;
pub mod gdpai_blackboard;
pub mod plan_tree;
pub mod precondition;
pub mod requirement;
pub mod scheduler;
pub mod sim_object_proxy;
pub mod snapshot;

struct GdPlanningAIExt;

#[gdextension]
unsafe impl ExtensionLibrary for GdPlanningAIExt {}
