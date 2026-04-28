# Preconditions, Requirements, and Provisions API Sketch

## Purpose

This document sketches a concrete API for separating:

- `Precondition`
- `Requirement`
- `Provision`

It is intended to follow the design direction in `PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md` while staying close to the current `Action` and Rust bridge architecture.

## Goals

The API sketch should satisfy these constraints:

- preserve the current `Action` authoring model as much as possible
- keep custom preconditions fully supported
- add planner-readable dependency metadata without relying on callback introspection
- fit the existing GDScript-to-Rust dictionary bridge
- use a built-in Rust-owned set of requirement/provision forms in the first pass
- expose thin built-in Godot wrapper classes over that Rust-owned schema
- avoid custom user-defined requirement/provision semantics unless later experience proves they are necessary
- support the first two target scenarios:
  - hunger / food chaining
  - `SpatialAction` decomposition into `GoTo + interaction`

## High-Level Shape

The proposed authoring surface is:

- `Action.get_validity_checks() -> Array[Precondition]`
- `Action.get_preconditions() -> Array[Precondition]`
- `Action.get_requirements() -> Array[RequirementSpec]`
- `Action.get_provisions() -> Array[ProvisionSpec]`
- `Action.get_action_cost(...) -> float`
- `Action.simulate_effect(...) -> void`

This preserves the current distinction between validity checks and preconditions, while adding separate channels for planner-readable backward dependencies.

## Built-In Schema Approach

### `Precondition`

Keep `Precondition` as the evaluative check type.

Expected role:

- truth evaluation against agent and world simulated state
- supports builtin and custom variants
- not assumed to be planner-inferable

No required conceptual changes beyond clarifying its role.

### Requirements and provisions

For the initial implementation, requirements and provisions should be exposed in GDScript through built-in wrapper classes that mirror the built-in Rust schema.

These are not intended to be open-ended extension points. They are typed wrappers over engine-defined forms.

This keeps the planner contract explicit while still giving users a safe, typed interface.

Conceptually:

```gdscript
func get_requirements() -> Array[RequirementSpec]:
	return []

func get_provisions() -> Array[ProvisionSpec]:
	return []
```

Each wrapper serializes to a built-in schema understood by the engine.

## Proposed Action API

### `Action`

Add two new methods to `Action`:

```gdscript
func get_requirements() -> Array[RequirementSpec]:
	return []

func get_provisions() -> Array[ProvisionSpec]:
	return []
```

### Intended meaning

- `get_validity_checks()`
  - cheap or mandatory checks for whether the action is even eligible
  - often scene-tree or object-liveness related

- `get_preconditions()`
  - evaluative checks against the current simulated state
  - may be opaque custom logic

- `get_requirements()`
  - planner-readable dependencies that should be satisfied earlier in the plan
  - should use built-in `RequirementSpec` forms

- `get_provisions()`
  - planner-readable facts/bindings this action contributes for later actions
  - should use built-in `ProvisionSpec` forms

## Proposed Godot Wrapper Types

### `RequirementSpec`

`RequirementSpec` should be a built-in wrapper type with static constructors for each supported requirement form.

Conceptually:

```gdscript
class_name RequirementSpec
extends RefCounted

func to_bridge_dict() -> Dictionary:
	push_error("RequirementSpec.to_bridge_dict must be overridden")
	return {}
```

Preferred usage:

```gdscript
RequirementSpec.binding_exists("held_item")
RequirementSpec.binding_equals("tool", "axe")
RequirementSpec.fact("at_target", [target])
RequirementSpec.binding_in_set("held_item", "edible_items")
```

The intended v1 constructor set is exactly:

- `RequirementSpec.binding_exists(name)`
- `RequirementSpec.binding_equals(name, value)`
- `RequirementSpec.binding_in_set(name, set_name)`
- `RequirementSpec.fact(name, args)`

### `ProvisionSpec`

`ProvisionSpec` should be a built-in wrapper type with static constructors for each supported provision form.

Conceptually:

```gdscript
class_name ProvisionSpec
extends RefCounted

func to_bridge_dict() -> Dictionary:
	push_error("ProvisionSpec.to_bridge_dict must be overridden")
	return {}
```

Preferred usage:

```gdscript
ProvisionSpec.binding("held_item", item_id)
ProvisionSpec.fact("at_target", [target])
```

The intended v1 constructor set is exactly:

- `ProvisionSpec.binding(name, value)`
- `ProvisionSpec.fact(name, args)`

## Minimal Requirement Kinds

The initial version should support a small number of explicit requirement forms.

### 1. Binding existence

Represents that a named binding must exist.

Example use:

- `held_item` must be bound before `EatHeldFoodAction` can simulate meaningfully

Sketch:

```gdscript
RequirementSpec.binding_exists("held_item")
```

### 2. Binding equality

Represents that a named binding must resolve to a specific value.

Example use:

- `tool = axe`

Sketch:

```gdscript
RequirementSpec.binding_equals("tool", "axe")
```

### 3. Fact requirement

Represents a structured predicate-like requirement.

Example use:

- `at_target(tree)`
- `door_open(chest_1)`

Sketch:

```gdscript
RequirementSpec.fact("at_target", [tree_ref])
```

### 4. Binding membership in a named set

Represents membership in a named domain or tag set.

Example use:

- `held_item` must be in `edible_items_for_action`

Sketch:

```gdscript
RequirementSpec.binding_in_set("held_item", "edible_items")
```

## Minimal Provision Kinds

### 1. Binding provision

Represents an action binding a name to a value.

Example use:

- pickup provides `held_item = banana`

Sketch:

```gdscript
ProvisionSpec.binding("held_item", banana_id)
```

### 2. Fact provision

Represents an action contributing a planner-relevant fact.

Example use:

- `GoToAction(tree)` provides `at_target(tree)`

Sketch:

```gdscript
{
	"kind": "fact",
	"fact_name": "at_target",
	"args": [tree_ref],
}
```

### 3. World/object fact provision

Represents a planner-relevant fact about world state or object state.

Example use:

- `door_open(chest)`

Sketch:

```gdscript
ProvisionSpec.fact("door_open", [chest_ref])
```

## Built-In Schema Notes

These built-in forms should be enough to cover the initial target scenarios and a large portion of common use cases.

We are intentionally not introducing custom user-defined requirement/provision semantics yet.

`RequirementSpec` and `ProvisionSpec` are themselves the preferred ergonomic layer for the first implementation.

The first pass should not add additional built-in constructors unless the benchmark scenarios show that this minimal set is insufficient.

## Suggested Bridge Format

### Action serialization

Current action serialization includes:

```gdscript
{
	"name": action.get_title(),
	"cost_callable": Callable(action, "get_action_cost"),
	"effect_callable": Callable(action, "simulate_effect"),
	"preconditions": _extract_preconditions(action.get_preconditions()),
	"validity_checks": _extract_preconditions(action.get_validity_checks()),
}
```

Proposed extension:

```gdscript
{
	"name": action.get_title(),
	"cost_callable": Callable(action, "get_action_cost"),
	"effect_callable": Callable(action, "simulate_effect"),
	"preconditions": _extract_preconditions(action.get_preconditions()),
	"validity_checks": _extract_preconditions(action.get_validity_checks()),
	"requirements": _extract_requirements(action.get_requirements()),
	"provisions": _extract_provisions(action.get_provisions()),
}
```

### Bridge helpers

Add:

```gdscript
func _extract_requirements(requirements: Array[RequirementSpec]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for requirement in requirements:
		extracted.append(requirement.to_bridge_dict())
	return extracted

func _extract_provisions(provisions: Array[ProvisionSpec]) -> Array[Dictionary]:
	var extracted: Array[Dictionary] = []
	for provision in provisions:
		extracted.append(provision.to_bridge_dict())
	return extracted
```

## Suggested Rust Data Model

The Rust side should use explicit enums rather than generic maps after parsing.

### Requirements

Sketch:

```rust
pub enum RequirementSpec {
    BindingExists { binding_name: String },
    BindingEquals { binding_name: String, value: Variant },
    BindingInSet { binding_name: String, set_name: String },
    Fact { fact_name: String, args: Vec<Variant> },
}
```

### Provisions

Sketch:

```rust
pub enum ProvisionSpec {
    Binding { binding_name: String, value: Variant },
    Fact { fact_name: String, args: Vec<Variant> },
}
```

These enums are the engine-owned built-in requirement/provision vocabulary for the initial implementation.

### Action spec

Extend the Rust-side action struct to include:

```rust
pub struct ActionSpec {
    pub name: String,
    pub cost_callable_id: i64,
    pub effect_callable_id: i64,
    pub preconditions: Vec<PreconditionSpec>,
    pub validity_checks: Vec<PreconditionSpec>,
    pub requirements: Vec<RequirementSpec>,
    pub provisions: Vec<ProvisionSpec>,
}
```

The exact struct name may differ from the current code, but this is the intended shape.

## Planner Branch State Sketch

The branch state needs planner metadata in addition to blackboard snapshots.

Suggested internal structures:

```rust
pub struct BranchBindings {
    pub values: HashMap<String, Variant>,
}

pub struct ActiveRequirement {
    pub requirement: RequirementSpec,
    pub introduced_by_action_index: i64,
}

pub struct SatisfiedProvision {
    pub provision: ProvisionSpec,
    pub provided_by_action_index: i64,
}
```

And the recursive branch context should track something like:

- unresolved requirements
- known bindings
- satisfied facts
- provenance for debugging

The exact storage can change later, but these concepts should be explicit in the planner.

## Matching Rules Sketch

The initial matching behavior should be simple and deterministic.

### Binding requirements

- `BindingExists(name)` is satisfied by:
  - an existing branch binding for `name`
  - or a `ProvisionBinding(name, value)`

- `BindingEquals(name, value)` is satisfied by:
  - an existing branch binding equal to `value`
  - or a `ProvisionBinding(name, value)`

- `BindingInSet(name, set_name)` is satisfied by:
  - a bound value whose membership in `set_name` is known or checkable

### Fact requirements

- `RequirementFact(name, args)` is satisfied by:
  - a matching `ProvisionFact(name, args)`

### No automatic callback inference

A false custom precondition does not automatically become a requirement.

If an action needs planner-inferable chaining, it must declare a requirement explicitly.

## Simulation Gating Sketch

We should add a planner-level distinction between:

- actions that can be simulated concretely now
- actions that are structurally relevant but still await requirement satisfaction

Possible initial rule:

- preconditions are evaluated only when their required bindings/facts are already resolved enough to make evaluation meaningful
- otherwise, requirements still participate in backward search, but the action's concrete effect is not yet used for goal satisfaction

This area will likely need iteration, but the API should support it.

## Example Sketch: `EatHeldFoodAction`

Conceptually:

```gdscript
func get_preconditions() -> Array[Precondition]:
	return [
		Precondition.agent_property_greater_than("hunger", 0.0),
	]

func get_requirements() -> Array[RequirementSpec]:
	return [
		RequirementSpec.binding_exists("held_item"),
		RequirementSpec.binding_in_set("held_item", "edible_items"),
	]

func get_provisions() -> Array[ProvisionSpec]:
	return []
```

Notes:

- no placeholder hunger reduction should be needed
- concrete simulation should use the resolved binding for `held_item`

## Example Sketch: `PickupAction`

Conceptually:

```gdscript
func get_preconditions() -> Array[Precondition]:
	return [
		Precondition.custom(func(agent_bb, _world_state):
			var held_item = agent_bb.get_property("held_item")
			return held_item == null or held_item == ""
		),
	]

func get_requirements() -> Array[RequirementSpec]:
	return [
		RequirementSpec.fact("at_target", [holdable_item]),
	]

func get_provisions() -> Array[ProvisionSpec]:
	return [
		ProvisionSpec.binding("held_item", holdable_item.item_id),
	]
```

Notes:

- no fake hunger reduction should happen here
- this action should only model pickup semantics

## Example Sketch: `GoToAction`

Conceptually:

```gdscript
class_name GoToAction
extends Action

var target_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable

func get_validity_checks() -> Array[Precondition]:
	return [
		Precondition.check_is_object_valid(target_location),
		Precondition.check_is_object_valid(interactable_attribs),
	]

func get_requirements() -> Array[RequirementSpec]:
	return []

func get_provisions() -> Array[ProvisionSpec]:
	return [
		ProvisionSpec.fact("at_target", [target_location]),
	]
```

Notes:

- cost and runtime behavior can borrow heavily from `SpatialAction`
- target interaction semantics should not live here

## Goal API Considerations

The initial rollout can leave goals unchanged if desired.

Current goal shape:

- reward
- desired state as `Array[Precondition]`

Options:

### Option A: keep goals unchanged initially

Pros:

- smaller first implementation
- fewer moving parts

Cons:

- goals remain evaluative only
- requirement-aware goals may need a later extension

### Option B: eventually add goal requirements

Possible future API:

- `Goal.get_desired_state() -> Array[Precondition]`
- `Goal.get_requirements() -> Array[RequirementSpec]`

Recommendation:

- do not require this for the first pass
- revisit after action-side chaining works

## Suggested First Implementation Scope

The smallest useful slice is:

- add `Action.get_requirements()` and `Action.get_provisions()`
- add built-in Godot wrapper classes `RequirementSpec` and `ProvisionSpec`
- serialize both over the bridge
- parse them into Rust enums
- support only:
  - binding existence requirements
  - binding equality requirements
  - binding-in-set requirements
  - fact requirements
  - binding provisions
  - fact provisions
- validate with:
  - hunger example
  - one `GoTo + interaction` example

This should be enough to prove the overall direction.

## Open Questions

### Should `args` allow raw `Object` references?

Probably yes at the bridge boundary if the existing bridge already supports object references in variants, but this needs validation.

If object identity becomes too fragile, we may need stable object ids for facts and bindings.

### Should set membership be represented by strings or explicit predicate objects?

Start with strings.

This keeps bridge serialization simple. If needed later, richer domain descriptors can be introduced.

### Should bindings live only in planner metadata or also in the simulated blackboard?

Initial recommendation:

- planner bindings should be distinct from blackboard state
- concrete simulation may still write blackboard properties as before
- binding resolution should not depend on blackboard writes alone

This avoids reintroducing the original ambiguity.

## Summary

This API sketch proposes:

- keeping `Precondition` as the evaluative planning check abstraction
- adding separate requirement and provision channels using a built-in Rust-owned schema
- exposing built-in Godot wrapper classes `RequirementSpec` and `ProvisionSpec` as the preferred interface
- extending `Action` with `get_requirements()` and `get_provisions()` returning `RequirementSpec` / `ProvisionSpec`
- extending bridge serialization with explicit requirement/provision arrays
- representing requirement/provision kinds as explicit Rust enums owned by the planner
- validating the first implementation against food chaining and `GoTo + interaction`
