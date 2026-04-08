# Rust Test Coverage Analysis & Plan

**Last Updated**: April 7, 2026  
**Status**: Planning Phase

## Executive Summary

The Rust codebase has **32 unit tests** covering 6 of 13 source modules. Core engine modules (`planning_engine.rs`, `scheduler.rs`, `background_plan.rs`) have **zero tests**. This document outlines current coverage and provides an actionable integration test plan.

---

## Current Unit Test Coverage

### ✅ Modules WITH Tests (6 modules, 32 tests)

#### 1. `background_types.rs` — **12 tests**
**Lines**: 418 total  
**Test Focus**: PreconditionSpec evaluation logic

Tests:
- `has_property_true_when_exists`
- `has_property_false_when_missing`
- `equal_integer_matches`
- `equal_integer_fails_on_mismatch`
- `greater_than_comparison_works`
- `less_than_comparison_works`
- `cross_type_int_float_comparison`
- `targets_world_state_correctly`
- `custom_callback_returns_none`
- `callable_id_accessor`
- `dependent_object_ids_accessor`
- `builtin_has_empty_dependencies`

**Coverage**: Excellent for precondition evaluation, comparison operators, target selection

---

#### 2. `snapshot.rs` — **6 tests**
**Lines**: 288 total  
**Test Focus**: Snapshot serialization and cloning

Tests:
- `variant_snapshot_nil_preserves_type`
- `variant_snapshot_int_preserves_value`
- `variant_snapshot_float_preserves_value`
- `blackboard_snapshot_retrieves_stored_values`
- `blackboard_snapshot_missing_property_returns_none`
- `blackboard_snapshot_clone_creates_independent_copy`

**Coverage**: Good for basic snapshot operations, clone independence verified

---

#### 3. `plan_tree.rs` — **5 tests**
**Lines**: 151 total  
**Test Focus**: Best plan extraction algorithm

Tests:
- `single_action_plan_returned`
- `picks_lowest_cost_single_step_branch`
- `multi_step_chain_cumulates_cost`
- `picks_cheapest_multi_step_path`
- `empty_root_returns_zero_cost_empty_plan`

**Coverage**: Excellent for cost minimization and path selection

---

#### 4. `logger.rs` — **4 tests**
**Lines**: ~120 total  
**Test Focus**: Log level management

Tests:
- `from_u8_maps_all_levels`
- `from_u8_out_of_range_gives_debug`
- `level_ordering_matches_verbosity`
- `set_get_log_level_roundtrip`

**Coverage**: Complete for logging system

---

#### 5. `precondition.rs` — **3 tests**
**Lines**: ~295 total  
**Test Focus**: Operation parsing

Tests:
- `parse_all_known_operations`
- `parse_operation_is_case_insensitive`
- `parse_unknown_operation_defaults_to_has_property`

**Coverage**: Good for parsing, missing runtime evaluation tests

---

#### 6. `goal.rs` — **2 tests**
**Lines**: ~108 total  
**Test Focus**: Goal sorting

Tests:
- `goals_sort_descending_by_reward`
- `original_index_preserved_after_sort`

**Coverage**: Complete for sorting logic

---

### ❌ Modules WITHOUT Tests (7 modules, 0 tests)

#### 1. `action.rs` — **NO TESTS** ⚠️ HIGH PRIORITY
**Lines**: 148  
**Risk**: Core action handling untested

**Missing Coverage**:
- `ActionData::from_dict()` parsing with invalid dictionaries
- `get_cost()` with non-float callable returns
- `is_valid()` edge cases (empty validity checks, failing checks)
- `preconditions_satisfied()` with mixed precondition types
- `apply_effect()` callable invocation

**Recommended Tests**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn from_dict_handles_missing_cost_callable() { }
    
    #[test]
    fn from_dict_handles_missing_effect_callable() { }
    
    #[test]
    fn is_valid_returns_false_when_any_check_fails() { }
    
    #[test]
    fn preconditions_satisfied_requires_all_to_pass() { }
}
```

---

#### 2. `background_plan.rs` — **NO TESTS** ⚠️ CRITICAL
**Lines**: 317  
**Risk**: Async planning algorithm completely untested

**Missing Coverage**:
- `run_plan()` end-to-end with mock callbacks
- `build_plan_recursive()` recursion depth limiting
- Goal satisfaction short-circuit
- Action validity checks via callbacks
- Progress-toward-goal evaluation
- Channel-based callback mechanism

**Note**: This module is critical for background planning. Integration tests required.

---

#### 3. `gdpai_blackboard.rs` — **NO TESTS** ⚠️ HIGH PRIORITY
**Lines**: 225  
**Risk**: Core state management untested

**Missing Coverage**:
- `clone_for_simulation()` creates independent copies (lines 204-223)
- `set_property()` with GDPAI_OBJECTS special handling
- `get_proxy_in_group()` / `get_proxies_in_group()` queries
- `get_node_in_group()` / `get_nodes_in_group()` on simulation vs live blackboards
- `erase_property()` with GDPAI_OBJECTS cleanup
- `set_dict()` bulk replacement

**Recommended Tests**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn clone_for_simulation_creates_independent_copy() {
        // CRITICAL: Verify mutations don't affect original
    }
    
    #[test]
    fn get_proxy_in_group_returns_first_match() { }
    
    #[test]
    fn get_node_in_group_returns_null_on_simulation_clone() { }
    
    #[test]
    fn set_property_gdpai_objects_rebuilds_object_map() { }
}
```

---

#### 4. `planning_engine.rs` — **NO TESTS** ⚠️ CRITICAL
**Lines**: 422  
**Risk**: Core GOAP engine untested

**Missing Coverage**:
- `build_plan()` end-to-end planning
- `plan()` goal prioritization by reward
- `try_build_plan_for_goal()` early exit when already satisfied
- `build_plan_recursive()` depth limiting at max_recursion
- `check_progress_toward_goal()` logic
- `is_goal_satisfied()` all-preconditions-met check
- `result_to_dict()` serialization

**Note**: This is the most critical module. Requires integration tests with mock GdPAIBlackboard.

---

#### 5. `scheduler.rs` — **NO TESTS** ⚠️ CRITICAL
**Lines**: 369  
**Risk**: Thread pool and job management untested

**Missing Coverage**:
- `submit_plan()` job creation and dispatch
- `process_callbacks()` callback draining and response dispatch
- Rayon thread pool initialization
- Concurrent job handling
- Agent validity checking before callback
- Callable registry per-job isolation
- `dispatch_callback()` with invalid callables

**Note**: Requires careful testing of thread safety and channel communication.

---

#### 6. `sim_object_proxy.rs` — **NO TESTS** ⚠️ MEDIUM PRIORITY
**Lines**: 133  
**Risk**: Object proxy logic untested

**Missing Coverage**:
- `from_object_data()` with malformed nodes
- `from_object_data()` when `get_groups()` call fails
- `from_object_data()` when `get_sim_properties()` call fails
- `is_in_group()` membership checks
- `get_property()` / `set_property()` / `has_property()` operations

**Recommended Tests**:
```rust
#[cfg(test)]
mod tests {
    #[test]
    fn from_object_data_handles_missing_get_groups() { }
    
    #[test]
    fn from_object_data_handles_missing_get_sim_properties() { }
    
    #[test]
    fn is_in_group_checks_membership_correctly() { }
}
```

---

#### 7. `lib.rs` — **NO TESTS**
**Lines**: 36  
**Risk**: Low (mostly module declarations)

**Coverage**: Extension registration is tested via integration with Godot.

---

## Integration Test Plan

### Directory Structure

Create the following structure in the Rust project:

```
c:\Godot\GdPlanningAI\addons\GdPlanningAI\rust\
├── src\
│   └── (existing modules)
└── tests\              # ← CREATE THIS
    ├── common\
    │   └── mod.rs      # Shared test utilities
    ├── planning_integration.rs
    ├── blackboard_integration.rs
    ├── scheduler_integration.rs
    └── action_chain_integration.rs
```

---

### Integration Test Files

#### 1. `tests/planning_integration.rs`
**Purpose**: Test full planning cycle without Godot runtime

**Test Scenarios**:
```rust
#[test]
fn full_planning_cycle_finds_optimal_path() {
    // Create blackboards with test data
    // Define 3+ actions with varying costs
    // Define goal with multiple preconditions
    // Run planning engine
    // Assert correct action chain selected
    // Assert total cost is minimal
}

#[test]
fn planning_handles_already_satisfied_goals() {
    // Setup blackboards where goal is already met
    // Run planner
    // Assert zero-cost empty action chain returned
}

#[test]
fn planning_handles_impossible_goals() {
    // Setup with unreachable goal (no valid action chain)
    // Run planner
    // Assert failure result with goal_index = -1
}

#[test]
fn planning_respects_max_recursion_depth() {
    // Create deeply nested goal requiring 15+ actions
    // Set max_recursion = 5
    // Run planner
    // Assert search terminates without stack overflow
}

#[test]
fn planning_prioritizes_higher_reward_goals() {
    // Define 3 goals: low reward (achievable), high reward (achievable)
    // Run planner
    // Assert high-reward goal is satisfied first
}
```

---

#### 2. `tests/blackboard_integration.rs`
**Purpose**: Test blackboard cloning and simulation isolation

**Test Scenarios**:
```rust
#[test]
fn clone_for_simulation_creates_independent_copy() {
    // Create blackboard with properties and objects
    // Clone for simulation
    // Mutate clone's properties
    // Mutate clone's object properties
    // Assert original unchanged
    // Assert clone has mutations
}

#[test]
fn simulation_clone_has_no_source_objects() {
    // Create blackboard with live Node references
    // Clone for simulation
    // Call get_node_in_group() on clone
    // Assert returns null
    // Call get_proxy_in_group() on clone
    // Assert returns proxy (not null)
}

#[test]
fn deep_cloning_of_sim_object_proxies() {
    // Create blackboard with SimObjectProxy
    // Clone blackboard
    // Mutate proxy properties in clone
    // Assert original proxy unchanged
}

#[test]
fn gdpai_objects_special_key_handling() {
    // Create blackboard
    // Set GDPAI_OBJECTS with array of mock nodes
    // Assert objects map populated
    // Query by group
    // Assert correct objects returned
}
```

---

#### 3. `tests/scheduler_integration.rs`
**Purpose**: Test async planning without Godot (mock callables)

**Test Scenarios**:
```rust
#[test]
fn submit_plan_dispatches_to_thread_pool() {
    // Create scheduler
    // Submit plan with mock agent
    // Assert job added to active_jobs
    // Assert Rayon spawns task
}

#[test]
fn process_callbacks_drains_pending_requests() {
    // Submit plan that generates callback requests
    // Call process_callbacks()
    // Assert requests consumed
    // Assert responses sent back
}

#[test]
fn concurrent_jobs_use_isolated_registries() {
    // Submit 2 plans with different callables
    // Process callbacks for both
    // Assert callable IDs don't collide
    // Assert correct callables invoked per job
}

#[test]
fn completed_jobs_deliver_results_to_agent() {
    // Submit plan
    // Process until completion
    // Assert _on_plan_ready called on agent
    // Assert job removed from active_jobs
}

#[test]
fn invalid_callable_returns_safe_default() {
    // Create callback request with freed object
    // Dispatch callback
    // Assert returns INFINITY for cost
    // Assert returns false for precondition
    // Assert returns unchanged snapshots for effect
}
```

---

#### 4. `tests/action_chain_integration.rs`
**Purpose**: Test action execution sequence

**Test Scenarios**:
```rust
#[test]
fn action_preconditions_evaluated_in_order() {
    // Create action with 3 preconditions (2 pass, 1 fail)
    // Evaluate preconditions_satisfied()
    // Assert returns false (short-circuit on failure)
}

#[test]
fn action_validity_checks_before_cost_calculation() {
    // Create action with failing validity check
    // Attempt to include in plan
    // Assert action skipped (cost never calculated)
}

#[test]
fn action_effects_modify_blackboard_state() {
    // Create action with effect callable
    // Apply effect to blackboard
    // Assert properties updated correctly
}

#[test]
fn multi_action_chain_executes_in_order() {
    // Plan returns [action_0, action_1, action_2]
    // Execute each in sequence
    // Assert state transitions correctly
    // Assert final state satisfies goal
}
```

---

### Shared Test Utilities (`tests/common/mod.rs`)

```rust
// Mock builders for testing without Godot

pub fn mock_blackboard() -> GdPAIBlackboard {
    // Create test blackboard with sample properties
}

pub fn mock_action(name: &str, cost: f64) -> ActionData {
    // Create action with mock callables
}

pub fn mock_goal(name: &str, reward: f64) -> GoalData {
    // Create goal with mock preconditions
}

pub fn mock_callable() -> Callable {
    // Create no-op callable for testing
}

pub fn assert_action_chain_valid(chain: &[i64], expected: &[i64]) {
    // Helper assertion for action chains
}
```

**Note**: These utilities will need creative solutions since Godot types (`Gd<T>`, `Callable`) require Godot runtime. Consider:
- Using `#[cfg(not(test))]` to conditionally compile Godot-dependent code
- Creating trait abstractions for testability
- Mocking with custom structs that implement similar interfaces

---

## Running Tests

### Basic Commands

```bash
# Run all tests (unit + integration)
cargo test

# Run only unit tests (in src/)
cargo test --lib

# Run only integration tests (in tests/)
cargo test --test '*'

# Run specific test file
cargo test --test planning_integration

# Run specific test
cargo test full_planning_cycle

# Show output (normally hidden for passing tests)
cargo test -- --nocapture

# Run tests in release mode (faster)
cargo test --release
```

### Continuous Integration

Add to `.github/workflows/rust-tests.yml`:
```yaml
name: Rust Tests
on: [push, pull_request]
jobs:
  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
      - run: cargo test --verbose
```

---

## Coverage Measurement

### Using Tarpaulin (Linux/macOS)

```bash
cargo install cargo-tarpaulin
cargo tarpaulin --out Html --output-dir coverage/
# Open coverage/index.html
```

### Using llvm-cov (Cross-platform)

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --html
# Open target/llvm-cov/html/index.html
```

### Target Coverage Goals

- **Unit Tests**: 80% line coverage for all modules
- **Integration Tests**: 100% coverage of critical paths in:
  - `planning_engine.rs`
  - `background_plan.rs`
  - `scheduler.rs`

---

## Development Dependencies

Add to `Cargo.toml`:

```toml
[dev-dependencies]
# Better assertions
assert_matches = "1.5"

# Property-based testing
proptest = "1.4"

# Async testing utilities
tokio-test = "0.4"

# Mock time for scheduler tests
mock_instant = "0.3"
```

---

## Testing Challenges & Solutions

### Challenge 1: Godot Types in Tests
**Problem**: `Gd<T>`, `Callable`, `Variant` require Godot runtime  
**Solutions**:
- Use conditional compilation: `#[cfg(not(test))]`
- Extract testable logic into pure Rust functions
- Create trait abstractions (`trait BlackboardLike`)
- Use builder pattern to inject dependencies

### Challenge 2: GDScript Callable Mocking
**Problem**: Can't create real `Callable` objects without Godot  
**Solutions**:
- Replace `Callable` with `Box<dyn Fn()>` in test builds
- Use feature flags: `#[cfg(feature = "godot-integration")]`
- Test serialization/deserialization only, not execution

### Challenge 3: Thread Pool Testing
**Problem**: Rayon spawns real threads, hard to deterministically test  
**Solutions**:
- Use `ThreadPool::spawn_fifo()` for deterministic ordering
- Test with `num_threads(1)` for sequential execution
- Use channels to synchronize and verify thread execution

### Challenge 4: Blackboard Cloning
**Problem**: `clone_for_simulation()` uses `Gd::new_gd()` (Godot-specific)  
**Solutions**:
- Extract clone logic to `impl Clone for InternalBlackboard`
- Test `BlackboardSnapshot` cloning (already works)
- Add unit tests for `HashMap` cloning (Rust std)

---

## Priority Test Implementation Order

### Phase 1: Unit Tests for Untested Modules (Week 1)
1. ✅ `action.rs` — 5 tests (parsing, validation, cost)
2. ✅ `sim_object_proxy.rs` — 3 tests (from_object_data edge cases)
3. ✅ `gdpai_blackboard.rs` — 4 tests (clone isolation, queries)

### Phase 2: Integration Test Framework (Week 2)
1. ✅ Set up `tests/` directory structure
2. ✅ Create `tests/common/mod.rs` with mock builders
3. ✅ Solve Godot-type mocking challenge
4. ✅ Write first passing integration test

### Phase 3: Critical Path Integration Tests (Week 3)
1. ✅ `tests/planning_integration.rs` — 5 tests
2. ✅ `tests/blackboard_integration.rs` — 4 tests
3. ✅ Measure initial coverage baseline

### Phase 4: Async & Concurrency Tests (Week 4)
1. ✅ `tests/scheduler_integration.rs` — 5 tests
2. ✅ `tests/action_chain_integration.rs` — 4 tests
3. ✅ Final coverage report (target: 80%+)

---

## Success Metrics

- **Unit Test Count**: 32 → 60+ (increase by ~28 tests)
- **Module Coverage**: 6/13 → 13/13 (100% module coverage)
- **Integration Tests**: 0 → 18+ scenarios
- **Line Coverage**: Unknown → 80%+ (measure with tarpaulin)
- **CI/CD**: Automated test runs on every commit

---

## Notes & Considerations

### Known Limitations
- Full end-to-end tests require Godot runtime (out of scope for pure Rust tests)
- GDScript callback testing is limited to mock verification
- Thread timing tests may be flaky (use generous timeouts)

### Future Enhancements
- Benchmark suite for planning performance (`cargo bench`)
- Fuzz testing for precondition evaluation (`cargo fuzz`)
- Property-based tests for plan optimality (`proptest`)
- Regression test suite from real-world planning failures

### Related Documents
- `THREADING_PLAN.md` — Background planning architecture
- `ARCHITECTURE_DESIGN.md` — Overall system design
- `BUILD_SYSTEM.md` — Cross-compilation and build process

---

## Appendix: Test Template

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    // Helper to create test fixtures
    fn setup() -> TestFixture {
        TestFixture { /* ... */ }
    }
    
    #[test]
    fn descriptive_test_name() {
        // Arrange
        let fixture = setup();
        
        // Act
        let result = function_under_test(&fixture);
        
        // Assert
        assert_eq!(result.expected_field, expected_value);
    }
    
    #[test]
    #[should_panic(expected = "specific error message")]
    fn test_error_condition() {
        // Test that should panic
    }
}
```

---

**End of Document**
