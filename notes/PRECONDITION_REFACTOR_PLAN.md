# Precondition Refactoring Plan

## Problem Statement

Current precondition handling is inefficient for the Rust bridge:

1. **Redundant Callable creation**: `_create_property_precondition` builds a full GDScript Callable that's never used when Rust handles evaluation - only metadata is needed
2. **Fragile bridge logic**: 8-way if-chain in `gdpai_rust_bridge.gd:91-97` checking each operation type
3. **Mixed concerns**: `Precondition` stores both `eval_func` AND metadata, but they're mutually exclusive
4. **Implicit custom detection**: Methods like `agent_has_object_data_of_group` create Callables without setting operation, relying on `Operation.NA` default

## Proposed Solution: Separate Types

Split into dedicated classes based on evaluation strategy:

### Base Class (Abstract Interface)

```gdscript
class_name Precondition
extends RefCounted

var is_satisfied: bool = false

func evaluate(agent: GdPAIBlackboard, world: GdPAIBlackboard) -> bool:
    if is_satisfied: return true
    is_satisfied = _do_evaluate(agent, world)
    return is_satisfied

func _do_evaluate(_agent, _world) -> bool:
    push_error("Override in subclass")
    return false

func copy_for_simulation() -> Precondition:
    push_error("Override in subclass")
    return null
```

### PreconditionBuiltin (Pure Data)

```gdscript
class_name PreconditionBuiltin
extends Precondition

enum Target { AGENT, WORLD_STATE }
enum Op { HAS_PROPERTY, EQUAL, NOT_EQUAL, GT, GTE, LT, LTE }

var target: Target
var operation: Op
var property: String
var value: Variant

func _init(t: Target, op: Op, prop: String, val: Variant = null) -> void:
    target = t
    operation = op
    property = prop
    value = val

func _do_evaluate(agent, world) -> bool:
    # Rust handles this during planning
    # GDScript implementation for fallback/debugging only
    var source = target_to_source(agent, world)
    match operation:
        Op.HAS_PROPERTY: return property in source.get_dict()
        Op.EQUAL: return source.get_property(property) == value
        Op.NOT_EQUAL: return source.get_property(property) != value
        Op.GT: return source.get_property(property) > value
        Op.GTE: return source.get_property(property) >= value
        Op.LT: return source.get_property(property) < value
        Op.LTE: return source.get_property(property) <= value
    return false
```

### PreconditionCustom (Pure Callable)

```gdscript
class_name PreconditionCustom
extends Precondition

var eval_func: Callable

func _init(fn: Callable) -> void:
    eval_func = fn

func _do_evaluate(agent, world) -> bool:
    return eval_func.call(agent, world)

func copy_for_simulation() -> Precondition:
    return PreconditionCustom.new(eval_func)
```

## Simplified Bridge

```gdscript
func _extract_precondition(precond: Precondition) -> Dictionary:
    if precond is PreconditionBuiltin:
        return {
            "kind": "builtin",
            "target": _target_to_string(precond.target),
            "operation": _op_to_string(precond.operation),
            "property_name": precond.property,
            "value": precond.value,
            "is_satisfied": precond.is_satisfied,
        }
    else:  # PreconditionCustom
        return {
            "kind": "custom",
            "eval_callable": precond.eval_func,
            "is_satisfied": precond.is_satisfied,
        }
```

## Static Helper API (Unchanged from User Perspective)

```gdscript
# In Precondition class (or dedicated factory)

static func agent_property_greater_than(prop: String, val: Variant) -> PreconditionBuiltin:
    return PreconditionBuiltin.new(PreconditionBuiltin.Target.AGENT, PreconditionBuiltin.Op.GT, prop, val)

static func agent_property_equal_to(prop: String, val: Variant) -> PreconditionBuiltin:
    return PreconditionBuiltin.new(PreconditionBuiltin.Target.AGENT, PreconditionBuiltin.Op.EQUAL, prop, val)

# ... other built-in helpers

static func custom(fn: Callable) -> PreconditionCustom:
    return PreconditionCustom.new(fn)
```

## Rust Side Changes

```rust
pub enum PreconditionKind {
    Builtin {
        target: PreconditionTarget,
        operation: PreconditionOp,
        property: String,
        value: Option<Variant>,
    },
    Custom {
        callable: Callable,
    },
}
```

## Benefits Comparison

| Aspect | Current | Refactored |
|--------|---------|------------|
| Memory | Every Precondition has unused Callable OR unused metadata | Only what's needed |
| Bridge logic | 8-way if-chain checking operations | Single type check |
| Type safety | Runtime detection via operation==NA | Compile-time via class |
| Extensibility | Add new op → update 3 places | Add new op → one enum |
| Clarity | Mixed concerns in one class | Single responsibility |

## Migration Path

1. **Create new classes** alongside existing `Precondition`
   - `precondition_builtin.gd`
   - `precondition_custom.gd`
   
2. **Update bridge** to handle both old and new types during transition

3. **Update static helpers** to return new types

4. **Deprecate old Precondition** after all usage migrated

5. **Remove old code** once transition complete

## Files to Modify

- `addons/GdPlanningAI/scripts/refcounteds/precondition.gd` → split into 3 files
- `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd` → simplify extraction logic
- `addons/GdPlanningAI/rust/src/precondition.rs` → update to use kind-based dispatch

## Open Questions

- Should `Precondition` remain as base class or become a factory namespace?
- How to handle `copy_for_simulation()` for builtins (they're immutable, just copy is_satisfied?)
- Any other precondition types needed beyond builtin/custom?
