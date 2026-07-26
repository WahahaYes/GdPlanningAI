# Documentation Guidelines

Code documentation standards for GdPlanningAI.

## General Principles

### No Decorative Comment Headers

Never use comment borders as section headers in any language. If a file is large enough to need section headers, consider splitting it into smaller files.

**GDScript (bad):**
```gdscript
# ---------------------------------------------------------------------------
# Section Name
# ---------------------------------------------------------------------------
```

**GDScript (good):**
```gdscript
## Brief description of section purpose.
```

**Rust (bad):**
```rust
// ---------------------------------------------------------------------------
// Section Name
// ---------------------------------------------------------------------------
```

**Rust (good):**
```rust
/// Module or item documentation.
```

Keep comments purposeful. Use docstrings for documentation, simple comments for implementation notes.

---

## GDScript

### Class Documentation

Place `##` docstring immediately after `class_name`/`extends`. Use `[ClassName]` to link related classes. Use `[br]` for paragraph breaks.

```gdscript
class_name MyAction
extends Action
## Brief description.[br]
##[br]
## Additional context. References [OtherClass] and [member property_name].
```

### Variable Declarations

Always use explicit type annotations. Never use type inference (`:=`).

```gdscript
var hunger_value: float = 20.0
var agent_name: String = ""
@export var cooldown_window: float = 30.0
```

### Variable Documentation

Document `@export` and significant non-exported variables with `##` directly above. Describe semantic meaning, not implementation.

```gdscript
## Seconds between shakes.
@export var cooldown_window: float = 30.0

## Whether tree is on cooldown.
var is_on_cooldown: bool = false
```

### Function Documentation

All functions get `##` docstrings. Use `[param name]` for parameters, `[code]...[/code]` for inline code, `[b]...[/b]` for critical constraints.

```gdscript
## Computes action cost during Rust planning simulation.[br]
##[br]
## Access simulated objects via [code]world_state.get_object_for(obj)[/code].[br]
##[br]
## [b]Do not use await from this method.[/b]
func get_action_cost(
	_agent_blackboard: GdPAIBlackboard,
	_world_state: GdPAIBlackboard,
) -> float:
	return 0
```

### Function Spacing

Two blank lines between function definitions.

```gdscript
## First function.
func first() -> void:
	pass


## Second function.
func second() -> void:
	pass
```

### Override Annotations

Mark abstract base class method implementations with `# Override`. Use for contractual overrides like `Action.get_action_cost()` or `Goal.compute_reward()`.

```gdscript
# Override
func get_provided_actions() -> Array[Action]:
	return []
```

---

## Rust

### Module Documentation

Use `//!` at file top. Describe purpose and public interface.

```rust
//! Background planning on [`BlackboardSnapshot`]s.
//!
//! Send-safe counterpart for async planning. Uses [`CallbackRequest`] channels.
```

### Type Documentation

Use `///` for structs, enums, functions. Cross-reference with ``[`TypeName`]``.

```rust
/// Action data from GDScript.
#[derive(Clone, Debug)]
pub struct ActionData {
    pub name: String,
    /// Callable for cost (agent_bb, world_bb)
    pub cost_callable: Callable,
}
```

### Function Documentation

Reference types with ``[`TypeName`]``. Document parameter semantics.

```rust
/// Entry point for background planning.
pub fn run_plan(
    agent: BlackboardSnapshot,
    world: BlackboardSnapshot,
    // ...
)
```

### Naming Conventions

- Types: `PascalCase` (`ActionData`, `PlanResult`)
- Functions/variables: `snake_case` (`run_plan`, `cost_callable`)
- Constants: `SCREAMING_SNAKE_CASE`

### Visibility

Explicit `pub` or private. Prefer private by default.

---

## Docstring Quick Reference

| Language | Tag | Purpose |
|----------|-----|---------|
| GDScript | `[param name]` | Parameter reference |
| GDScript | `[member name]` | Class member reference |
| GDScript | `[ClassName]` | Type reference |
| GDScript | `[code]...[/code]` | Inline code |
| GDScript | `[b]...[/b]` | Bold/emphasis |
| GDScript | `[br]` | Line break |
| Rust | ``[`Type`]`` | Type cross-reference |
| Rust | ``[`crate::mod::Type`]`` | Full path reference |