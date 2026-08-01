# GdPlanningAI Authoring Guide

This guide explains how to create goals, actions, world objects, and behavior configurations for agents. It assumes you understand GOAP basics: goals define what an agent wants, actions define what it can do, and the planner chains them together.

## The Mental Model

You describe **what matters** (goals, state changes) not **how to sequence**. The planner searches backward from a goal's desired state, finding actions whose simulated effects satisfy open preconditions, then recursively satisfying those actions' requirements through provisions from earlier actions. Built-in preconditions let the planner evaluate matches natively during search. Custom callables are opaque: the planner must callback to GDScript for every candidate, which can hinder discovery performance and debuggability. Use built-ins wherever the property you care about lives on the agent blackboard, world blackboard, or a world object in a known group.

## Goals: What the Agent Wants

A goal has two jobs: compute a reward that scales with urgency, and declare the state that counts as "satisfied." The reward function runs once per planning request when the agent submits a new plan. A hunger goal returns the current hunger value (0–100), so a starving agent prioritizes eating over wandering. A fire-maintenance goal returns a fixed reward when any campfire's fuel drops below a threshold, otherwise zero, so the agent ignores the fire when it's healthy.

The desired state uses preconditions. For agent-internal properties like hunger, use `agent_property_less_than("hunger", 15)`. For world-object properties like campfire fuel, use `world_object_property_geq_than("CampfireObject", "current_fuel", 60)`. This built-in lets the planner evaluate the condition directly on simulated `SimObjectProxy` snapshots during search, so it can discover that `AddFuelAction` (which increments `current_fuel` in its simulation) satisfies the goal. No custom callable needed.

If you genuinely need logic that no built-in expresses, e.g. a distance check against a remembered origin, a compound condition across multiple unrelated properties, etc., use `Precondition.custom(callable)` sparingly. For object-dependent checks that must survive the object being freed, use `Precondition.custom_with_deps(callable, [object_refs])` so the planner validates liveness before invoking.

## Actions: What the Agent Can Do

An action exposes five declarative surfaces plus runtime execution:

**Validity checks** are hard gates evaluated once against the initial state during candidate discovery. They answer "is this action even meaningful right now?"; target exists, agent has the required property, navmesh reachable. They do not participate in backward chaining. Use `Precondition.check_is_object_valid(ref)` for object liveness; avoid custom callables here.

**Preconditions** are dynamic needs the planner *chains backward* to satisfy. If an action needs stamina > 20, declare `agent_property_greater_than("stamina", 20)` and the planner will search for a predecessor that restores stamina. Keep these to built-ins matching property names your simulation writes.

**Requirements** are symbolic dependencies; "I need X before I run." They match against provisions from earlier actions purely by name and type, no simulation required. `binding_exists("held_item")` means some prior action must provide a `held_item` binding. `fact("at_target", [location])` means a `GoToAction` (or any action providing that fact) must come first. `binding_in_set("held_item", "Food")` matches any provision whose bound object belongs to the "Food" group.

**Provisions** are what this action contributes for later actions. `binding("held_item", "wood")` publishes the item ID. `fact("is_food", [])` asserts a tag. `fact_wildcard("at_target")` (used by `GoToAction`) means "I satisfy any `at_target` fact requirement, capturing the concrete location at planning time."

**Simulation** runs on isolated snapshots (`agent_bb`, `world_bb`) during search. It must be synchronous, deterministic, and mutate only the snapshots. Property names here *must* match the precondition names in goals and other actions; that's how the planner connects them. A common pattern: when an action consumes a binding (like `held_item`), clear it in simulation (`agent_bb.set_property("held_item", "")`) so the planner knows a predecessor must provide it. For actions whose requirement isn't bound yet during backward search (e.g., `EatHeldFood` before pickup), simulate an optimistic effect using a conservative estimate so the planner recognizes relevance, then let forward validation enforce the real requirement.

**Cost** estimates planning-time expense. It can be a constant, a distance heuristic, or an async callback if you need expensive computation. Return `INF` (or `float('inf')` in GDScript) to disable the action in the current context; e.g., `AddFuelAction` returns infinite cost when the campfire is already full.

## Chaining: Requirements Meet Provisions

The planner matches requirements to provisions symbolically during search. A `Binding` provision satisfies `BindingExists` (any value), `BindingEquals` (exact value), or `BindingInSet` (value's object in the named group). A `Fact` provision satisfies an identical `Fact` requirement (name and args). A `FactWildcard` provision satisfies any `Fact` with the same name, capturing the requirement's args as the binding; this is how `GoToAction` universally satisfies navigation requirements.

Design principle: requirements express *what* you need, provisions express *what you give*. The planner handles *who gives what to whom*. You rarely need to think about insertion order; the placer inserts each predecessor immediately before the earliest consumer it satisfies.

## World Objects: Exposing Actions and State

A `GdPAIObjectData` subclass lives on a scene node, holds `@export` references to its `GdPAIInteractable` and `GdPAILocationData` children, and returns actions from `get_provided_actions()` that capture `self`. Its `get_sim_properties()` dictionary defines the snapshot the planner works with; properties like `current_fuel`, `berries_remaining`, `hunger_value`. Groups returned by `get_group_labels()` enable `WorldObjectProxy` preconditions: `"CampfireObject"` lets a goal check `current_fuel` on any campfire; `"FoodSource"` lets a hunger goal find berries.

Actions provided by the object typically require `at_target` (via the object's location data) and provide `held_item` bindings plus `is_food` facts. The object's own runtime script (separate from the data class) handles visual updates, cooldowns, and actual `queue_free()` of picked-up items; the action's `perform_action` mirrors this but operates on the live scene.

## Behavior Composition

A `GdPAIBehaviorConfig` bundles goals, actions, and property updaters into a reusable module. `_populate(goals, actions, updaters)` appends to the passed arrays. An agent's `GdPAIAgentConfig` resource holds an array of behavior configs. At runtime, the agent merges all behaviors: goals, actions, and updaters from each. Multiple behaviors can share `GoToAction`; the planner deduplicates by instance. Property names must not conflict across behaviors (two behaviors writing `hunger` would fight). The config resource is assigned in the inspector; no code wiring needed.

## Built-in Base Classes

The addon (`addons/GdPlanningAI/`) provides base classes you extend to create behaviors; the example behaviors in `examples/behaviors/` show how to use them:

**GdPAIBehaviorConfig** is the base class for all behaviors. Override `_populate(goals, actions, updaters)` to append your goals, actions, and property updaters.

**GdPAIAgentConfig** is a resource that holds an array of behavior configs plus planning strategy (`ON_INTERVAL`, `CONTINUOUS`, `ON_DEMAND`, `ON_INTERVAL_FORCED`) and blackboard plan.

**PropertyUpdater** is the base for things that modify agent properties over time (hunger decay, stamina regen). Override `initialize(agent)` and `update_properties(agent, delta)`.

**GoToAction** is the default navigation action. It provides `fact_wildcard("at_target")` so any interaction action can require `fact("at_target", [location])` and the planner chains navigation automatically. For a custom movement system (grid, flying, swimming), subclass `GoToAction` or write your own action that provides `fact_wildcard("at_target")` and computes cost appropriately.

**Action** and **Goal** are the base classes you extend for all custom actions and goals.

These are the building blocks. The examples demonstrate concrete implementations (HungerBehaviorConfig, WanderBehaviorConfig, CampfireBehaviorConfig) but your project defines its own behaviors by subclassing the addon's base classes.

## Common Patterns

**Inventory → Consumption**: Pickup actions provide `binding("held_item", id)` + `fact("is_food", [])`. Eat action requires `binding_exists("held_item")` + `fact("is_food", [])`. Planner chains GoTo → Pickup → Eat automatically.

**Navigation Glue**: `GoToAction` provides `fact_wildcard("at_target")`. Every interaction action requires `fact("at_target", [my_location])`. One navigation action serves all interactions.

**Object State Goals**: Use `world_object_property_geq_than(group, property, threshold)` in the goal's desired state. The matching action modifies that property in `simulate_effect`. Planner discovers the chain without custom code.

**Optimistic Simulation**: When an action's requirement isn't bound yet during backward search, simulate a conservative effect (e.g., fixed hunger reduction) so the planner sees relevance. Forward validation will reject the plan if the real bound value doesn't match.

**Instance-Specific Validity**: Use `check_is_object_valid(ref)` in validity checks. For property checks on a specific instance (not any in a group), you currently need a custom precondition with deps; `WorldObjectProxy` is group-based.

## Debugging

The interactive debugger tab was removed during the Rust refactor and has not been reintroduced yet. For now, the engine exposes the search tree as a text dump: `GdPAIPlanScheduler.get_debug_tree(agent)` returns a human-readable rendering of the agent's most recent planning job. Enable debug logging via `GdPAIAutoload.get_scheduler().set_log_level(3)` to see candidate discovery, simulation steps, and forward validation in the output log.

**Empty plan, goal not satisfied**: No action provides a needed provision, or a validity check filters everything out. Check that your action's provisions match the requirement types exactly (Binding vs Fact, group names match).

**Plan fails at runtime**: `perform_action` logic diverges from `simulate_effect`. Keep them in sync; simulation is the planner's model of reality.

**Wrong action chosen**: Cost function doesn't reflect true expense, or a missing precondition lets an invalid action look cheaper.

**Plan too deep / cycles**: A requirement chain has no provider. The cycle guard (`open_requirements > max_depth`) will prune it. Add the missing provision or fix the requirement.

## Quick Reference: Built-in Preconditions

| Target | Operations | |--|| | Agent | `has_property`, `equal`, `not_equal`, `greater_than`, `geq_than`, `less_than`, `leq_than` | | WorldState | Same operations, on world blackboard properties | | WorldObjectProxy (group) | Same operations, evaluated on each object in the group; succeeds if ANY matches |

Construct via `Precondition.agent_property_less_than("hunger", 15)`, `Precondition.world_object_property_geq_than("CampfireObject", "current_fuel", 60)`, etc.

## Quick Reference: Requirement / Provision Constructors

| Need | Requirement | Provided By | ||-|-| | Any binding | `binding_exists("name")` | `binding("name", value)` | | Specific value | `binding_equals("name", value)` | `binding("name", value)` | | Object in group | `binding_in_set("name", "Group")` | `binding("name", obj_ref)` where obj in Group | | Location fact | `fact("at_target", [location])` | `fact_wildcard("at_target")` (GoTo) or `fact("at_target", [location])` | | Custom fact | `fact("name", [args])` | `fact("name", [args])` |

## Quick Reference: Property Names in Examples

| Domain | Agent Property | World Object Property | |--|-|-| | Hunger | `hunger` (0–100) | `hunger_value` on `FoodObject` | | Campfire | (none) | `current_fuel` on `CampfireObject` | | Inventory | `held_item` (String) | `item_id` on `HoldableObject` | | Location | `GdPAILocationData` on agent | `GdPAILocationData` on object |

*This guide reflects the current framework. As built-ins expand (distance checks, virtual properties), patterns will simplify further.*
