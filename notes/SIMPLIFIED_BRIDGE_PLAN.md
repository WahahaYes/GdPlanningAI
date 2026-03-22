# Simplified Rust Bridge Implementation Plan

**Date**: March 15, 2026
**Goal**: Eliminate double serialization by passing Godot objects directly to Rust

---

## Executive Summary

The current implementation serializes `GdPAIBlackboard`, `Action`, and `Precondition` objects to dictionaries in GDScript, then deserializes them to Rust structs, then re-serializes for callbacks. This is:

1. **Slow**: Multiple serialization passes per planning cycle
2. **Complex**: Duplicate data structures in both languages
3. **Fragile**: Easy for Rust/GDScript structs to drift out of sync

**Solution**: Use godot-rust's `Gd<RefCounted>` to hold references to Godot objects and call their methods dynamically.

---

## Architecture Comparison

### Current (Complex)
```
GDScript                          Rust
─────────────────────────────────────────────────────────────
GdPAIBlackboard ──serialize──> Dictionary ──deserialize──> BlackboardState
     │                                                           │
     │                      callback round-trip:                 │
     └────────────────────<──deserialize──Dictionary──serialize<┘
```

### Proposed (Simple)
```
GDScript                          Rust
─────────────────────────────────────────────────────────────
GdPAIBlackboard ──────────────> Gd<RefCounted>
     │                               │
     │         direct method calls:  │
     │<──────────────────────────────┘
```

---

## Implementation Phases

### Phase 1: Rust Data Structure Refactor

**Files to modify:**
- `rust/src/blackboard.rs` - Remove `BlackboardState`, `ObjectDataSnapshot` entirely
- `rust/src/bridge.rs` - Simplify to work with `Gd<RefCounted>`
- `rust/src/planning_engine.rs` - Accept objects directly
- `rust/src/action.rs` - Store action references instead of serialized definitions
- `rust/src/precondition.rs` - Store precondition objects instead of definitions

**New approach:**
```rust
// planning_engine.rs
#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct RustPlanningEngine {
    max_recursion: usize,
    // Store callables directly - no ID registry needed
    precondition_callbacks: Dictionary,  // precondition_id -> Callable
    action_effects: Dictionary,          // action_uid -> Callable
    #[base]
    base: Base<RefCounted>,
}

#[godot_api]
impl RustPlanningEngine {
    #[func]
    fn build_plan(
        &mut self,
        agent_blackboard: Gd<RefCounted>,  // Direct object reference
        world_state: Gd<RefCounted>,
        actions: Array<Gd<RefCounted>>,    // Array of Action objects
        goals: Array<Gd<RefCounted>>,      // Array of Goal objects
    ) -> Dictionary {
        // Work with objects directly via dynamic calls
    }
}
```

### Phase 2: Callback Mechanism Simplification

**Current approach (complex):**
1. GDScript registers callable with ID
2. Rust stores ID
3. Rust calls GDScript bridge with ID
4. GDScript bridge looks up callable by ID
5. GDScript bridge invokes callable

**New approach (simple):**
1. GDScript passes `Callable` directly to Rust
2. Rust stores `Callable` 
3. Rust calls `Callable.call()` directly

```rust
// precondition.rs - evaluate custom preconditions
impl PreconditionHandler {
    fn evaluate_custom(
        &self,
        callable: &Callable,
        agent_blackboard: &Gd<RefCounted>,
        world_state: &Gd<RefCounted>,
    ) -> bool {
        let result = callable.call(&[
            agent_blackboard.to_variant(),
            world_state.to_variant(),
        ]);
        result.try_to::<bool>().unwrap_or(false)
    }
}
```

### Phase 3: GDScript Bridge Simplification

**Files to modify:**
- `scripts/gdpai_rust_bridge.gd` - Remove serialization helpers

**Simplified bridge:**
```gdscript
class_name GdPAIRustBridge
extends RefCounted

var planning_engine: RefCounted

func _init():
    planning_engine = RustPlanningEngine.new()

## Main planning entry point - pass objects directly
func build_plan(
    agent: GdPAIAgent,
    actions: Array[Action],
    goals: Array[Goal]
) -> Dictionary:
    # No serialization - pass objects directly
    return planning_engine.build_plan(
        agent.blackboard,           # GdPAIBlackboard object
        agent.world_node.get_world_state(),  # GdPAIBlackboard object
        _extract_action_callables(actions),  # Array of {uid, callable} dicts
        _extract_goal_data(goals, agent)     # Array of goal data
    )

## Extract action UIDs and their effect callables
func _extract_action_callables(actions: Array[Action]) -> Array[Dictionary]:
    var result: Array[Dictionary] = []
    for action in actions:
        result.append({
            "uid": action.uid,
            "cost_callable": Callable(action, "get_action_cost"),
            "effect_callable": Callable(action, "simulate_effect"),
            "preconditions": _extract_preconditions(action.get_preconditions()),
            "validity_checks": _extract_preconditions(action.get_validity_checks()),
        })
    return result

## Extract precondition data with callables for custom ones
func _extract_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
    var result: Array[Dictionary] = []
    for precond in preconditions:
        var dict = {
            "target": precond.target,
            "operation": precond.operation,
            "property_name": precond.property_name,
            "value": precond.value,
            "is_satisfied": precond.is_satisfied,
        }
        # For custom preconditions, include the callable
        if precond.eval_func.is_valid():
            dict["eval_callable"] = precond.eval_func
        result.append(dict)
    return result
```

### Phase 4: State Simulation Approach

**Decision**: Rust manages simulation state internally to avoid repeated translations.

- Rust extracts needed values into internal HashMap for simulation
- Original objects remain untouched
- Only needed for planning, not execution
- Extract once at planning start, simulate many times during search

```rust
// planning_engine.rs
struct SimulationState {
    // Internal Rust state for fast simulation
    properties: HashMap<String, Variant>,
    objects: HashMap<String, ObjectSimState>,
}

impl SimulationState {
    /// Extract state from Godot object once
    fn from_blackboard(bb: &Gd<RefCounted>) -> Self {
        let props_dict: Dictionary = bb.call("get_dict", &[]).try_to().unwrap();
        // Extract into HashMap for fast Rust operations
    }
    
    /// Apply simulated effect
    fn apply_effect(&mut self, effect: &Callable) {
        // Call effect with our simulation state
        // Effect modifies a copy that we provide
    }
}
```

---

## Migration Steps (Ordered)

Since this is an active migration, we can make large changes directly without deprecation paths.

### Step 1: Refactor Rust data structures
- [ ] Add `SimulationState` struct in Rust for internal state tracking
- [ ] Replace `build_plan` signature to accept `Gd<RefCounted>` directly
- [ ] Remove `BlackboardState`, `ObjectDataSnapshot` from Rust

### Step 2: Implement direct object handling
- [ ] Implement `SimulationState::from_blackboard()`
- [ ] Implement `SimulationState::apply_effect()` with callable
- [ ] Implement precondition evaluation with callables

### Step 3: Update GDScript bridge
- [ ] Replace `build_plan` to pass objects directly
- [ ] Remove all serialization helpers
- [ ] Test with new Rust implementation

### Step 4: Performance validation
- [ ] Benchmark implementation
- [ ] Verify no regressions in planning quality
- [ ] Document performance characteristics

---

## API Changes

### RustPlanningEngine (Rust → GDScript)

**Before:**
```gdscript
planning_engine.build_plan(
    serialized_blackboard_dict,
    serialized_world_state_dict,
    serialized_actions_array,
    serialized_goals_array
)
```

**After:**
```gdscript
planning_engine.build_plan(
    agent.blackboard,              # GdPAIBlackboard (RefCounted)
    world_node.get_world_state(),  # GdPAIBlackboard (RefCounted)
    actions_array,                 # Array[Action] objects
    goals_array                    # Array[Goal] objects
)
```

### Callback Registration

**Before:**
```gdscript
# Separate registration step
planning_engine.register_callbacks(
    _evaluate_precondition_callback,
    _simulate_effect_callback
)
# Then ID-based lookup during planning
```

**After:**
```gdscript
# Callables passed directly with actions/preconditions
# No separate registration needed
```

---

## Risk Mitigation

| Risk | Mitigation |
|------|------------|
| Dynamic calls slower than native | Profile first; Godot's `call()` is optimized |
| Type safety reduced | Use typed wrappers where possible |
| Breaking existing code | Keep old API during migration with deprecation warning |
| Callable lifetime issues | Store strong references in Rust during planning |

---

## Expected Benefits

1. **Performance**: Eliminate 2+ serialization passes per planning cycle
2. **Maintainability**: Single source of truth (GDScript objects)
3. **Simplicity**: ~200 lines of serialization code removed
4. **Flexibility**: Users can add new property types without Rust changes

---

## Open Questions

1. **Thread safety**: Planning may run on background thread - need to verify `Callable` works across threads or marshal appropriately
2. **Deep copy semantics**: When simulating effects, how do we handle nested object references?
3. **Error handling**: How to report errors from dynamic calls back to GDScript?

---

## References

- [godot-rust Callable docs](https://godot-rust.github.io/docs/gdext/master/godot/prelude/struct.Callable.html)
- [godot-rust Dynamic calls guide](https://godot-rust.github.io/book/godot-api/functions.html#dynamic-calls)
- [Gd smart pointer docs](https://godot-rust.github.io/docs/gdext/master/godot/obj/struct.Gd.html)
