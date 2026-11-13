# Documentation Guidelines

How code is documented in GdPlanningAI. `make necro` checks the mechanical rules (comment borders, doc length, the `:=` ban, spacing and formatting); this page covers the conventions behind them.

## Principles

- **Keep docs short.** Names and types carry most of the meaning. Say what a reader can't see from the signature (contracts, gotchas, why) and skip docs that only restate a name.
- **Hard limits** on each contiguous doc block, examples included: **10 lines** for a file, class or type header (`##` after `extends`, `//!`, or `///` on a struct, enum or trait), **6 lines** for anything else. Longer explanations belong in `docs/`, worked examples in `examples/`.
- **No decorative headers.** Never use comment borders as section headers; a file that needs sections should be split. Use docstrings for documentation and plain comments for implementation notes.

## GDScript

- Put the class docstring (`##`) right after `class_name` / `extends`. Document `@export` and significant variables with `##` directly above, by meaning rather than implementation.
- Document functions whose signature doesn't say it all.
- Mark implementations of base-class contracts (`Action.get_action_cost()`, `Goal.compute_reward()`) with `# Override`.
- Always annotate types explicitly; never infer with `:=`.

```gdscript
class_name MyAction
extends Action
## Picks up the nearest [GdPAIObjectData] in the Food group.[br]
## Fails if [member reach] is exceeded while walking.

## Units the agent may drift from the item before giving up.
@export var reach: float = 2.0


# Override
## Returns the walk time; [code]INF[/code] rules the action out.
func get_action_cost(_agent_blackboard: GdPAIBlackboard, _world_state: GdPAIBlackboard) -> float:
	return 1.0
```

## Rust

- Use `//!` at the top of a file for its purpose and public surface, and `///` on items. Cross-reference with `` [`TypeName`] `` (or `` [`crate::mod::Type`] ``).
- Prefer private by default; make `pub` explicit.

```rust
//! Background planning on [`BlackboardSnapshot`]s, answering callbacks over [`CallbackRequest`] channels.

/// One planning job's search; moved between the worker and the main thread, never shared.
pub struct PlannerEngine { /* ... */ }
```

## Docstring tags

| Language | Tag                                        | Purpose                    |
| -------- | ------------------------------------------ | -------------------------- |
| GDScript | `[ClassName]`, `[member name]`             | Type and member references |
| GDScript | `[param name]`                             | Parameter reference        |
| GDScript | `[code]...[/code]`, `[b]...[/b]`           | Inline code, emphasis      |
| GDScript | `[br]`                                     | Line break                 |
| Rust     | `` [`Type`] ``, `` [`crate::mod::Type`] `` | Type cross-reference       |
