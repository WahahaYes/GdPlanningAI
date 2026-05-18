# Bug: Action Callable Resolution & Cost Mismatch

## Symptoms
- Integration tests in `test_requirements_provisions.gd` fail with cost mismatches (e.g., `[51.0] expected to equal [51.5]`).
- The planner returns the default cost (1.0) instead of the value calculated by the action's GDScript `get_action_cost`.
- Some plans fail entirely because actions with no effects (but valid indices) were having their effects "simulated" against a null/empty registry entry, resulting in wiped blackboards.

## Root Causes Identified

### 1. Sentinel Value Collision
The Rust `ActionSpec` used `usize` for `cost_callable_id` and `effect_callable_id`, using `0` as a sentinel for "no callable provided." However, `0` is a valid index in the per-job `callable_registry`. This meant:
- The first registered callable in any job was ignored/skipped by "if id != 0" checks.
- Actions that genuinely had no callable were sometimes matched against index 0 of a different action.

### 2. Strict Type Conversion
Rust's `try_to::<f64>()` fails if Godot returns an `int` variant (which it often does for whole numbers like `1`, `20`, etc.). This caused `dispatch_callback` to return `f64::INFINITY`, leading the planner to prune valid actions.

### 3. State Wipe on "Empty" Effects
In `BranchExpander`, the forward state accumulation loop was calling `apply_effect` even for actions that didn't have one. Because of the sentinel issue, it often called index 0, which might return an empty blackboard snapshot if the registry lookup failed or returned a default, effectively wiping the agent's state midway through a chain.

## Fixes Implemented

### 1. Option-based IDs
- Changed `ActionSpec` to use `Option<usize>` for callable fields.
- Updated `scheduler.rs` to correctly populate `Some(id)` or `None`.
- Updated `expander.rs` and `simulation.rs` to only perform RPC calls if the ID is `Some`.

### 2. Robust Numeric Conversion
- Modified `dispatch_callback` in `scheduler.rs` to attempt `try_to::<f64>()` first, falling back to `try_to::<i64>()` and casting to `f64`.

### 3. Unified Simulation Logic
- Extracted state mutation and validity checking into `planner/simulation.rs` to ensure `forward_validate` and `BranchExpander` are always in sync.
