# Preconditions, Requirements, and Provisions Plan

## Purpose

This document records the design decision to separate action planning metadata into three distinct concepts:

1. `Precondition`: evaluative checks against the current simulated state.
2. `Requirement`: planner-understandable dependencies that must be satisfied earlier in the chain.
3. `Provision`: planner-understandable facts or bindings an action contributes for later actions.

This design is intended to replace placeholder-based state dependency chaining with a more general dependency propagation system.

For the initial implementation, requirements and provisions will use a built-in engine-defined schema owned by the Rust planner. We are not planning custom user-facing requirement or provision classes in the first pass.

The preferred Godot-facing interface for this built-in schema will be thin wrapper classes with static constructors, such as `RequirementSpec.binding_exists("held_item")` and `ProvisionSpec.binding("held_item", item_id)`.

The initial constructor surface we intend to support is:

- `RequirementSpec.binding_exists(name)`
- `RequirementSpec.binding_equals(name, value)`
- `RequirementSpec.binding_in_set(name, set_name)`
- `RequirementSpec.fact(name, args)`
- `ProvisionSpec.binding(name, value)`
- `ProvisionSpec.fact(name, args)`

## Problem Summary

The current planner mixes two different concerns inside preconditions and action simulation:

- checking whether an action is valid in the current simulated state
- determining what earlier actions are needed to make a later action meaningful

Examples:

- `EatHeldFoodAction` depends on `held_item` being set by a prior pickup action.
- `PickupAction` and similar `SpatialAction`-derived actions implicitly depend on the agent first reaching the target object.

The current placeholder-based workaround is a symptom of the planner lacking a first-class way to represent unresolved dependencies.

## Design Decision

We will separate the concepts instead of trying to make one `Precondition` type serve all roles.

### Why we are separating them

`Precondition`, `Requirement`, and `Provision` have different responsibilities:

- `Precondition`
  - answers: "is this true right now in the simulated state?"
  - can be builtin or arbitrary custom logic
  - is primarily evaluative

- `Requirement`
  - answers: "what must be made true earlier in the plan?"
  - must be structured enough for the planner to reason backward about it
  - is primarily inferential

- `Provision`
  - answers: "what planner-relevant fact, relation, or binding does this action make available?"
  - must be structured enough to match against requirements
  - is primarily connective between actions

If these remain merged, users will reasonably expect all preconditions to be inferable by the planner. That is not true for arbitrary custom callables.

## Core Principle

Planner reasoning must not depend on interpreting arbitrary GDScript callbacks.

Instead:

- callbacks are used to evaluate truth in a concrete state
- structured requirements and provisions are used to infer predecessor/successor action relationships

This allows user-written actions to remain highly customizable without requiring the planner to reverse-engineer arbitrary logic.

## Relationship to Custom Preconditions

Custom preconditions remain fully supported.

However, they should be treated as evaluative guards unless the action also declares planner-readable requirements and provisions.

Examples of good uses for custom preconditions:

- target object still exists
- weather or time-of-day restrictions
- agent has a valid animation controller
- arbitrary gameplay rule checks
- reachability validation against live navigation state

Examples of things that should not rely only on custom preconditions:

- needing to hold a valid item before consuming it
- needing to be at a target before interacting with it
- needing a specific tool before harvesting or opening something
- needing a binding between a selected target and a later action

## Responsibilities of Each Concept

### Preconditions

Preconditions remain the place for simulated-state evaluation.

Expected characteristics:

- may be builtin or custom
- may depend on agent blackboard and world state
- may be opaque to the planner
- continue to be used for validity and correctness checks

Examples:

- `hunger > 0`
- `held_item == null`
- `target object is still valid`
- `world contains at least one FoodObject`

### Requirements

Requirements define planner-readable dependencies that can be propagated backward.

Expected characteristics:

- must have explicit structure
- must be serializable to Rust without embedding arbitrary logic
- may represent facts, bindings, or relational constraints
- may optionally carry narrow matching metadata

Initial categories to support:

- binding existence
  - example: `held_item` must be bound
- binding equality
  - example: `tool = axe`
- fact requirements
  - example: `at_target(tree)`
- membership/tag requirements
  - example: `held_item` must be edible

### Provisions

Provisions describe planner-readable facts or bindings supplied by an action.

Expected characteristics:

- must have explicit structure
- must be matchable against requirements
- may derive from action parameters or object metadata
- are distinct from full simulated side effects

Initial categories to support:

- binding provision
  - example: `held_item = banana`
- fact provision
  - example: `at_target(tree)`
- world/object fact provision
  - example: `door_open(door)`

## Design Constraints

### 1. Concrete simulation remains important

We are not replacing simulated blackboard effects.

Concrete simulation is still needed for:

- cost evaluation
- goal progress evaluation
- pruning
- runtime faithfulness

The new requirement/provision system exists to decide when an action is worth exploring and when its effect can be simulated meaningfully.

### 2. Custom preconditions are not planner-readable by default

The planner may evaluate a custom precondition as true or false, but it cannot generally infer:

- what variable is missing
- which earlier action could satisfy it
- what values would make it true
- how to backpropagate it safely

This is the main reason requirements and provisions must be distinct structured data.

### 3. We are deferring helper API design

For now, we will design explicit internal and authoring concepts first.

Convenience helpers can be added later once the core semantics are stable.

### 4. Requirement and provision kinds will be built in initially

The initial planner implementation will support a built-in set of requirement and provision forms defined by the engine.

This means:

- Rust owns the matching semantics
- GDScript authors use built-in Godot wrapper classes that mirror the built-in schema
- custom user-defined requirement/provision semantics are out of scope for the first pass

If the built-in forms prove insufficient later, we can revisit a more extensible lowering model. That is not the current target.

## Proposed API Direction

The authoring model should eventually separate these three channels clearly.

At the conceptual level:

- `get_preconditions()`
- `get_requirements()`
- `get_provisions()`

These names are tentative, but the separation is the important decision.

The initial implementation should prefer explicitness over convenience.

In particular:

- `Precondition` remains a typed evaluative abstraction
- requirements and provisions should initially be authored through built-in wrapper classes such as `RequirementSpec` and `ProvisionSpec`
- those wrapper classes should serialize to the Rust-owned built-in schema
- helper APIs beyond those built-in wrappers can be added later if they improve ergonomics without changing semantics

For v1, we should treat the constructor surface above as intentionally minimal and sufficient for the main target scenarios:

- food chaining via held-item bindings
- `GoTo + interaction` composition via planner facts like `at_target(target)`
- common tool/item gating via binding equality or membership-in-set

## Planner Model Changes

The planner should carry more than just simulated blackboard snapshots.

Each branch should also track planner metadata such as:

- unresolved requirements
- currently satisfied requirements
- known bindings
- provision provenance for debugging

This planner metadata should be used to determine whether an action:

- satisfies an unresolved requirement
- introduces new requirements worth pursuing
- can be concretely simulated with the bindings currently available
- contributes no useful progress and should be pruned

## Matching Rules

At a high level:

- requirements match against provisions
- preconditions evaluate against concrete state
- simulation runs against concrete state once enough bindings or facts are available

Initial matching should remain conservative and explicit.

We should avoid early over-generalization such as:

- symbolic execution
- automatic inference from custom callbacks
- broad wildcard matching without clear author intent

## Example: Food Chaining

### Current issue

`EatHeldFoodAction` needs a held food item, but backward exploration currently simulates its effect before pickup has occurred.

### With separation

`EatHeldFoodAction`:

- preconditions
  - hunger is above zero
- requirements
  - `held_item` must be bound
  - `held_item` must be edible for this action
- provisions
  - none relevant for earlier chaining

`PickupAction(food)`:

- preconditions
  - hands are empty
- requirements
  - agent is at the food target
- provisions
  - `held_item = food.item_id`

`GoToAction(food)`:

- preconditions
  - target is valid/reachable
- requirements
  - none or only generic movement constraints
- provisions
  - `at_target(food)`

This allows the planner to find `GoTo -> Pickup -> Eat` without placeholder hunger reduction values.

## Example: Replacing SpatialAction Compositionally

A major benchmark and design goal is to replace `SpatialAction` as a bundled abstraction with composition.

### Desired model

`SpatialAction` currently bundles:

- navigation cost heuristics
- planning-time simulated teleportation
- arrival handling
- drift checks
- target interaction semantics

Instead, we want:

- `GoToAction(target)` to provide `at_target(target)`
- object-specific interaction actions to require `at_target(target)`
- interaction actions to focus only on domain behavior

Examples:

- `PickupAction(item)` requires `at_target(item)` and provides `held_item = item_id`
- `ShakeTreeAction(tree)` requires `at_target(tree)` and provides changed world state
- `OpenChestAction(chest)` requires `at_target(chest)` and provides `chest_open(chest)`

This is one of the clearest demonstrations of why requirements and provisions should be separate from preconditions.

## Implementation Phases

### Phase 1: Define data model and bridge types

Add separate serializable representations for:

- preconditions
- requirements
- provisions

Tasks:

- define the built-in requirement/provision schema used across GDScript and Rust
- define Rust-side matching semantics for that built-in schema
- define built-in Godot wrapper classes for the supported schema forms
- extend the GDScript-to-Rust bridge to serialize them independently
- keep existing planner behavior unchanged where possible during this phase

### Phase 2: Planner branch metadata

Extend planner branch state to track:

- unresolved requirements
- satisfied provisions or bindings
- requirement satisfaction provenance

Tasks:

- add planner-side storage for requirements and bindings
- add matching logic between action provisions and branch requirements
- add debug logging for requirement introduction, matching, and failure

### Phase 3: Requirement-aware exploration

Update recursive expansion so an action may be explored if it:

- makes concrete goal progress
- satisfies one or more unresolved requirements
- introduces meaningful predecessor requirements worth pursuing

Tasks:

- revise progress heuristics to consider requirement progress in addition to direct goal progress
- ensure pruning remains conservative enough to avoid search explosion

### Phase 4: Binding-aware simulation ✅ COMPLETED

Teach the planner to distinguish between:

- actions that can be concretely simulated now
- actions whose effect depends on unresolved bindings or facts

Implementation details:

- `PlanContext` now carries `accumulated_provisions: &[ProvisionSpec]` tracking provisions from actions already selected in the branch
- `PendingAction` tracks `child_provisions` and `was_concretely_simulated` flag for each branch
- Added `get_unsatisfied_requirements()` helper to identify which requirements aren't satisfied by accumulated provisions
- Actions with **satisfied requirements**: get full cost callback + effect simulation
- Actions with **unresolved requirements**: still explored (so predecessors can satisfy them), but use placeholder cost (1.0) and skip effect simulation
- Provisions accumulate through the branch, allowing later actions to see if their requirements are now satisfied

**Critical: Re-simulation on descent**

When descending into a pending branch, if an action was previously deferred (placeholder simulation) but its requirements are now satisfied by accumulated provisions, it is **re-simulated** before recursing:

1. Re-run `call_get_cost()` with actual callback → updated cost
2. Re-run `call_apply_effect()` with actual callback → updated agent/world snapshots
3. Mark as concretely simulated for the child context

This ensures that when `Pickup → Eat` chains are explored, the `Eat` action gets accurate cost and effects after `Pickup` provides `held_item`, rather than keeping the placeholder values.

This allows the planner to explore action chains where earlier actions provide bindings (e.g., `held_item`) that later actions require, without needing placeholder-driven simulation hacks.

### Phase 5: Migrate examples

Migrate the hunger example first.

Tasks:

- remove placeholder hunger reduction from `EatHeldFoodAction`
- stop reducing hunger in `PickupAction` simulation
- declare requirements and provisions explicitly
- validate that food chaining still plans successfully

Then prototype compositional movement.

Tasks:

- create a generic `GoToAction(target)`
- migrate one object interaction away from `SpatialAction`
- verify that `GoTo + interaction` composes correctly

### Phase 6: Cleanup and helper APIs

Only after the core semantics are proven:

- add convenience constructors/helpers
- add documentation for common patterns
- evaluate whether any existing precondition helpers should produce requirement/provision metadata automatically

## Non-Goals for the Initial Implementation

The first implementation should not try to solve everything at once.

Not in scope initially:

- symbolic execution
- inferring structured dependencies from arbitrary custom callbacks
- automatic DSL extraction from existing custom preconditions
- redesigning the entire planner around theorem-proving semantics

## Primary Risks

### Search explosion

Adding requirement-aware exploration could increase the number of viable branches.

Mitigation ideas:

- conservative matching rules
- indexing actions by provisions
- branch scoring that prefers satisfying currently unresolved requirements
- strict logging and instrumentation during rollout

### User confusion between checks and dependencies

If the distinction is not documented well, users may assume any false precondition can be backpropagated.

Mitigation ideas:

- explicit naming in the API
- examples that show the separation clearly
- documentation that states custom preconditions are evaluative unless paired with requirements/provisions

### Over-generalizing too early

Trying to solve all dependency patterns in the first pass could stall implementation.

Mitigation ideas:

- support a small number of requirement/provision kinds first
- validate them against hunger and `GoTo + interaction`
- expand only after concrete success

## Benchmarks and Validation

We should evaluate both planner correctness and authoring value.

### Benchmark 1: Food chaining

Scenario variations:

- multiple food objects
- different hunger levels
- mixed nutrition values
- reachable and unreachable food
- edible and non-edible held items

Success criteria:

- no placeholder values needed for eat simulation
- no fake hunger reduction during pickup simulation
- plan quality remains correct
- planner performance remains acceptable

### Benchmark 2: `SpatialAction` replacement

Scenario variations:

- pickup interaction
- tree shaking or harvesting interaction
- chest/lever/openable interaction
- multiple interactables at different distances

Success criteria:

- `GoToAction` composes correctly with interaction actions
- less duplicated movement logic in user actions
- plan quality is as good as or better than bundled `SpatialAction`
- authored interaction actions become simpler

### Benchmark 3: Multi-step dependency chain

Suggested chain:

- get tool
- go to target
- use tool on target
- collect output
- use collected output elsewhere

Success criteria:

- requirements and provisions can chain through multiple actions
- planner can resolve both bindings and location-based facts
- search does not become unstable

## Immediate Next Steps

1. Define the minimal requirement/provision schema.
2. Represent requirements and provisions on the GDScript side using built-in wrapper classes over that schema.
3. Add bridge serialization and Rust parsing for separate requirement/provision channels.
4. Instrument planner logging around requirement matching.
5. Migrate the hunger example as the first proof of concept.
6. Prototype `GoToAction + interaction` as the second proof of concept.

The first pass should not expand the built-in surface beyond these constructors unless one of the benchmark scenarios proves they are insufficient.

## Decision Summary

We are explicitly choosing to:

- separate preconditions, requirements, and provisions
- keep custom preconditions as first-class evaluative guards
- avoid relying on custom preconditions for planner inference
- use a built-in Rust-owned requirement/provision set for the initial implementation
- use built-in Godot wrapper classes such as `RequirementSpec` and `ProvisionSpec` as the preferred interface
- avoid custom user-defined requirement/provision semantics in the first pass
- defer helper API design until the core semantics are implemented and validated
- use hunger chaining and `SpatialAction` decomposition as the primary validation targets
