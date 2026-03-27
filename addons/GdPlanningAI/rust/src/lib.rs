//! GdPlanningAI Rust extension library.
//!
//! Provides a forward-chaining GOAP (Goal-Oriented Action Planning) engine
//! exposed to Godot 4 via GDExtension. Users interact with it entirely from
//! GDScript; no Rust knowledge is required.
//!
//! # Public Godot classes
//! - [`gdpai_blackboard::GdPAIBlackboard`] — key/value store for agent and world state.
//! - [`sim_object_proxy::SimObjectProxy`] — simulation snapshot of a world object.
//! - [`planning_engine::RustPlanningEngine`] — the planning engine itself.
//!
//! # Internal types
//! - [`action::ActionData`], [`goal::GoalData`], [`precondition::PreconditionHandler`]
//!   are deserialized from GDScript bridge dictionaries and used only inside Rust.

use godot::prelude::*;

#[macro_use]
pub mod logger;
pub mod action;
pub mod background_plan;
pub mod background_types;
pub mod gdpai_blackboard;
pub mod goal;
pub mod plan_tree;
pub mod planning_engine;
pub mod precondition;
pub mod scheduler;
pub mod sim_object_proxy;
pub mod snapshot;

struct GdPlanningAIExt;

#[gdextension]
unsafe impl ExtensionLibrary for GdPlanningAIExt {}
