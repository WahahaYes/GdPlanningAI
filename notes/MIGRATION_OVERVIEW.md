# GdPlanningAI — GDScript Migration Overview

## Context

The planning algorithm has been extracted into a Rust/GDExtension library (`RustPlanningEngine`).
Rust now owns: forward-chaining GOAP search, blackboard state cloning, precondition evaluation,
goal sorting, cost comparison, and action effect simulation. GDScript callables are invoked from
Rust for the user-customizable parts (cost, effect, custom precondition evaluation).

The GDScript layer has not yet been updated to reflect this new division of responsibility.
Several classes are carrying weight they no longer need to carry. This document summarizes
what exists, what the problems are, and the guiding principles for the migration.

---

## Current GDScript File Inventory

### scripts/ (top-level)
| File | Role |
|---|---|
| `gdpai_rust_bridge.gd` | Serializes Action/Goal/Precondition GDScript objects into Rust-readable dicts, calls `RustPlanningEngine`. `class_name GdPAIRustBridge`. Only ever used by `GdPAIAgent`. |
| `gdpai_blackboard_plan.gd` | Resource that holds an initial `Dictionary` and generates a `GdPAIBlackboard` from it. Simple and clean. |

### scripts/nodes/
| File | Role |
|---|---|
| `gdpai_agent.gd` | Main agent node. Manages planning timers, invokes the bridge, executes the action chain via pre/perform/post lifecycle. |
| `gdpai_world_node.gd` | Scene-level node. Scans the scene tree for `GdPAIObjectData` nodes and provides a `GdPAIBlackboard` world state. |
| `gdpai_object_data.gd` | Base node for world objects. Provides group labels, `get_provided_actions()`, and `get_sim_properties()`. Users subclass this. |
| `gdpai_interactable.gd` | Subclass of `GdPAIObjectData`. Adds `max_interaction_distance` and `max_drift_from_plan` sim properties. |
| `gdpai_location_data.gd` | Subclass of `GdPAIObjectData`. Wraps a `Node2D` or `Node3D` to provide `position`/`rotation` sim properties. Has a backing-variable bug (see below). |

### scripts/refcounteds/
| File | Role |
|---|---|
| `action.gd` | Base class for all actions. Defines 7 methods across two concerns: **planning** (`get_validity_checks`, `get_action_cost`, `get_preconditions`, `simulate_effect`) and **execution** (`pre_perform_action`, `perform_action`, `post_perform_action`). Also provides `set/get/erase/has_state` helpers. |
| `goal.gd` | Base class for goals. Provides `compute_reward` and `get_desired_state`. Clean and minimal. |
| `precondition.gd` | Static factory class + base type. Exposes ~15 static factory methods (e.g. `agent_property_greater_than`). |
| `precondition_builtin.gd` | Concrete `Precondition` subclass for property-based checks. Holds `target`, `operation`, `property`, `value`. Contains its own `_do_evaluate` that duplicates Rust's evaluation logic. |
| `precondition_custom.gd` | Concrete `Precondition` subclass wrapping a user `Callable`. |
| `property_updater.gd` | Per-frame blackboard update logic. Users subclass and override `update_properties`. |
| `spatial_action.gd` | `Action` subclass that handles navigation to a `GdPAILocationData` target. Manages `NavigationAgent2D/3D`, proximity, drift, stuck detection. |

### scripts/resources/
| File | Role |
|---|---|
| `gdpai_agent_config.gd` | Serializable `Resource`. Holds `PlanningStrategy` enum, `planning_interval`, `blackboard_plan`, and `behavior_configs`. |
| `gdpai_behavior_config.gd` | Resource bundle: lists of `goals`, `self_actions`, and `property_updaters`. Users subclass and override `_self_init`. |

---

## Identified Problems

### 1. `GdPAIRustBridge` is a public class with no public use
`GdPAIRustBridge` has `class_name` and is documented as "not intended for direct use." It is only
ever instantiated inside `GdPAIAgent._ready()`. Exposing it as a named class pollutes the user
namespace and forces users to understand an internal detail of the architecture. It should become
a private implementation detail.

### 2. Double validity-check evaluation
`GdPAIAgent._compute_valid_self_actions()` and `_compute_worldly_actions()` pre-filter the action
lists by running validity checks in GDScript *before* passing them to Rust. Rust then re-runs the
same validity checks during its recursive search. This means every validity check callable fires
at least twice per planning cycle. The pre-filter adds complexity and the `await` it requires
makes `_query_world_state_and_plan` async, complicating `_process`.

The Rust engine already handles invalid actions gracefully (skips them). The GDScript pre-filter
should be removed.

### 3. `await` inside `_process`
`GdPAIAgent._process` calls `await _query_world_state_and_plan()`. Since `_process` runs every
frame and the coroutine may not complete within a single frame, this can result in multiple
concurrent planning coroutines. It is also unnecessary now that the Rust engine is synchronous
and the `await` on validity checks is being removed.

### 4. `PreconditionBuiltin._do_evaluate` duplicates Rust logic
The GDScript `_do_evaluate` in `PreconditionBuiltin` re-implements `has_property`, `equal`,
`greater_than`, etc. This logic lives (better) in Rust and is only used in the pre-planning
validity check pre-filter (which is being removed). After removing the pre-filter, the only
reason `_do_evaluate` is kept is for standalone precondition evaluation outside of planning —
a minor use case. The duplication should be acknowledged and the GDScript version removed or
reduced to a thin fallback.

### 5. `GdPAIRustBridge._extract_preconditions` uses `is PreconditionBuiltin` instanceof check
The bridge must distinguish builtin vs custom preconditions to know how to serialize them. This
couples the bridge tightly to the internal concrete types. If the serialization responsibility
were pushed down into the precondition classes themselves (via a `to_bridge_dict()` virtual
method), the bridge would not need to import or inspect the internal subtype.

### 6. `GdPAILocationData` backing variable bug
`GdPAILocationData.position` has a setter `set(val): position = val`. In Godot 4, this creates
infinite recursion because the setter calls itself. The getter fallback `return position` similarly
recurses when both node references are null. Both need a backing `_position` / `_rotation`
variable as a fallback store.

### 7. `GdPAIBehaviorConfig` shared-resource bug
`GdPAIBehaviorConfig` caches its `goals`, `self_actions`, and `property_updaters` in instance
variables and sets `_is_initialized = true` after the first `_self_init()`. Because it is a
`Resource`, Godot may share a single instance across multiple agents using the same `.tres` file.
The second agent would receive an already-initialized (and potentially stale) list. Each agent
must either duplicate the resource or the init must not cache.

### 8. `max_recursion` is in `GdPAIAgentConfig` but set on the engine
`test_rust_bridge.gd` sets `config.max_recursion = 4`, but `GdPAIAgentConfig` as defined in
`gdpai_agent_config.gd` has no `max_recursion` field — it is set directly on `RustPlanningEngine`.
This suggests a field is missing from the config resource, or the test is incorrectly setting it.

---

## Guiding Principles for the Migration

1. **Rust owns planning, GDScript owns scene interaction and execution.** GDScript should not
   re-implement or second-guess what Rust does. Remove duplicated evaluation logic.

2. **Minimize the user-facing namespace.** Classes that are implementation details of the
   framework (bridge, concrete precondition subclasses) should not have `class_name`. Users only
   need to know about the classes they subclass or instantiate directly.

3. **Synchronous by default.** Planning is now a single synchronous Rust call. Remove all `await`
   from the planning path. Validity checks must be synchronous.

4. **Serialization knowledge belongs to the type being serialized.** The precondition classes
   should know how to produce their own bridge dictionary, rather than the bridge inspecting their
   type. This is the single-responsibility principle applied to the bridge.

5. **Fix existing bugs during the migration.** The `GdPAILocationData` backing variable issue and
   the `GdPAIBehaviorConfig` shared-resource issue should be fixed alongside the architectural
   changes.

6. **Preserve the user-facing API surface wherever possible.** Users currently subclass `Action`,
   `Goal`, `Precondition`, `GdPAIObjectData`, `GdPAIBehaviorConfig`, and `PropertyUpdater`. The
   methods they override should remain stable. Internal plumbing changes should be invisible to
   game code.
