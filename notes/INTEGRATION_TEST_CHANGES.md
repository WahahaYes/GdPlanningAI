# Integration Test Setup - Implementation Guide

**Goal:** Set up Godot integration testing with GUT framework for testing the Rust GDExtension.

**Expected Outcome:** All 9 integration tests passing, verifying blackboard cloning and planning engine functionality.

---

## Prerequisites

- Godot 4.6.1 (or compatible version)
- Rust toolchain installed
- `godot-env` installed ([installation guide](https://github.com/chickensoft-games/GodotEnv))
- Git repository at a clean state before test setup

---

## Implementation Steps

Follow these steps **in order**. Each step includes verification to ensure you're on the right track.

### Step 1: Set Up godot-env for GUT Addon Management

**Why:** We use godot-env to manage third-party addons (like GUT) separately from our own addon code.

**1.1: Create `addons.jsonc` at project root**

If the file doesn't exist, create it. If it exists, add the GUT entry to the `addons` section:

```jsonc
// addons.jsonc
{
  "$schema": "https://chickensoft.games/schemas/addons.schema.json",
  "addons": {
    "gut": {
      "url": "https://github.com/bitwes/Gut",
      "checkout": "v9.6.0",
      "subfolder": "addons/gut"
    }
  }
}
```

**1.2: Install GUT addon using godot-env**

```bash
godot-env addons install
```

This will clone GUT to `addons/gut/` and create a `.addons/` cache directory.

**1.3: Verify installation**

Check that `addons/gut/` directory exists and contains GUT files:
```bash
ls addons/gut/
# Should show: gut_cmdln.gd, gut.gd, gui/, etc.
```

---

### Step 2: Configure .gitignore for Addon Management

**Why:** We need to track our own addon (`addons/GdPlanningAI/`) in git while ignoring third-party addons managed by godot-env.

**2.1: Update `.gitignore`**

Find the section that ignores addon files and replace with:

```gitignore
# Ignore third-party addons managed by godot-env
addons/gut/
.addons/

# Keep our own addon tracked in git
!addons/GdPlanningAI/
!addons/.editorconfig
```

**Important:** Remove any blanket `addons/*` ignore patterns, as they would ignore our own addon.

**2.2: Verify git tracking**

```bash
git status
# Should show addons/GdPlanningAI/ files as tracked
# Should NOT show addons/gut/ files
```

---

### Step 3: Create GUT Configuration

**Why:** GUT needs to know where test files are located and how to run.

**3.1: Create `.gutconfig.json` at project root**

```json
{
  "dirs": ["res://test"],
  "include_subdirs": true,
  "log_level": 2,
  "should_exit": true
}
```

**3.2: Verify Godot can see GUT**

```bash
godot --headless --import --quit
# Should import addons without errors
```

---

### Step 4: Create Test Infrastructure

**Why:** Provides convenient commands to run tests locally and in CI.

**4.1: Create `Makefile` at project root**

```makefile
.PHONY: test test-rust test-godot help

help:
	@echo "GdPlanningAI Test Commands"
	@echo "=========================="
	@echo "make test        - Run all tests (Rust + Godot)"
	@echo "make test-rust   - Run Rust tests only"
	@echo "make test-godot  - Run Godot integration tests"

test: test-rust test-godot

test-rust:
	cd addons/GdPlanningAI/rust && cargo test

test-godot:
	@echo "Importing Godot resources..."
	@godot --headless --import --quit
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit

test-godot-verbose:
	@echo "Importing Godot resources..."
	@godot --headless --import --quit
	@echo "Running tests..."
	godot --headless -s --path . addons/gut/gut_cmdln.gd -gexit -gpo
```

**Note:** The import step (`godot --headless --import --quit`) is critical - Godot must import addon resources before tests can run.

**4.2: Verify make targets**

```bash
make help
# Should display test command options
```

---

## Source Code Fixes

These fixes address bugs discovered when writing integration tests for the Rust-GDScript bridge.

### Fix 1: Export `clone_for_simulation` to GDScript

**Problem:** Method exists in Rust but isn't callable from GDScript.

**File:** `addons/GdPlanningAI/rust/src/gdpai_blackboard.rs`

**Location:** Line ~204

**Change:** Add `#[func]` attribute before the method declaration:

```rust
// BEFORE:
pub fn clone_for_simulation(&self) -> Gd<GdPAIBlackboard> {
    let mut new_bb = GdPAIBlackboard::new_gd();
    // ...
}

// AFTER:
#[func]
pub fn clone_for_simulation(&self) -> Gd<GdPAIBlackboard> {
    let mut new_bb = GdPAIBlackboard::new_gd();
    // ...
}
```

**Why:** The `#[func]` macro is required by gdext to generate FFI bindings for GDScript.

**Verification:**
```bash
cd addons/GdPlanningAI/rust
cargo build --release
# Should compile without errors
```

---

### Fix 2: Handle Untyped GDScript Arrays in Goal Parsing

**Problem:** GDScript uses untyped arrays by default (`[]`), but Rust expects typed arrays (`Array<VarDictionary>`). When parsing fails, goals get empty `desired_state`, causing the planner to think all goals are already satisfied.

**File:** `addons/GdPlanningAI/rust/src/goal.rs`

**Location:** Line ~45 (in `GoalData::from_dict` method)

**Change:** Replace the `desired_state` parsing block:

```rust
// BEFORE:
let desired_state = dict
    .get("desired_state")
    .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
    .map(|arr| {
        arr.iter_shared()
            .filter_map(|d| PreconditionHandler::from_dict(&d))
            .collect()
    })
    .unwrap_or_default();

// AFTER:
let desired_state = dict
    .get("desired_state")
    .and_then(|v| {
        // Try typed array first, fall back to untyped VarArray
        v.try_to::<Array<VarDictionary>>()
            .ok()
            .or_else(|| v.try_to::<VarArray>().ok().map(|arr| {
                let mut typed = Array::<VarDictionary>::new();
                for item in arr.iter_shared() {
                    if let Ok(dict) = item.try_to::<VarDictionary>() {
                        typed.push(&dict);
                    }
                }
                typed
            }))
    })
    .map(|arr| {
        arr.iter_shared()
            .filter_map(|d| PreconditionHandler::from_dict(&d))
            .collect()
    })
    .unwrap_or_default();
```

**Why:** Provides graceful fallback when GDScript passes untyped arrays.

---

### Fix 3: Handle Untyped Arrays in Action Parsing

**File:** `addons/GdPlanningAI/rust/src/action.rs`

**Change 3.1:** Update `preconditions` parsing (line ~60)

Apply the same VarArray fallback as in goal.rs:

```rust
// BEFORE:
let preconditions = dict
    .get("preconditions")
    .and_then(|v| v.try_to::<Array<VarDictionary>>().ok())
    .map(|arr| {
        arr.iter_shared()
            .filter_map(|d| PreconditionHandler::from_dict(&d))
            .collect()
    })
    .unwrap_or_default();

// AFTER:
let preconditions = dict
    .get("preconditions")
    .and_then(|v| {
        v.try_to::<Array<VarDictionary>>()
            .ok()
            .or_else(|| v.try_to::<VarArray>().ok().map(|arr| {
                let mut typed = Array::<VarDictionary>::new();
                for item in arr.iter_shared() {
                    if let Ok(dict) = item.try_to::<VarDictionary>() {
                        typed.push(&dict);
                    }
                }
                typed
            }))
    })
    .map(|arr| {
        arr.iter_shared()
            .filter_map(|d| PreconditionHandler::from_dict(&d))
            .collect()
    })
    .unwrap_or_default();
```

**Change 3.2:** Update struct definition (line ~12)

Change `validity_checks` from `Vec<PreconditionHandler>` to `Vec<Callable>`:

```rust
// BEFORE:
pub struct ActionData {
    pub name: String,
    pub cost_callable: Callable,
    pub effect_callable: Callable,
    pub preconditions: Vec<PreconditionHandler>,
    pub validity_checks: Vec<PreconditionHandler>,  // ← WRONG TYPE
}

// AFTER:
pub struct ActionData {
    pub name: String,
    pub cost_callable: Callable,
    pub effect_callable: Callable,
    pub preconditions: Vec<PreconditionHandler>,
    pub validity_checks: Vec<Callable>,  // ← CORRECT TYPE
}
```

**Change 3.3:** Update `validity_checks` parsing (line ~82)

```rust
// BEFORE:
let validity_checks = dict
    .get("validity_checks")
    .and_then(|v| v.try_to::<Array<Callable>>().ok())
    .map(|arr| arr.iter_shared().collect())
    .unwrap_or_default();

// AFTER:
let validity_checks = dict
    .get("validity_checks")
    .and_then(|v| {
        v.try_to::<Array<Callable>>()
            .ok()
            .or_else(|| v.try_to::<VarArray>().ok().map(|arr| {
                let mut typed = Array::<Callable>::new();
                for item in arr.iter_shared() {
                    if let Ok(callable) = item.try_to::<Callable>() {
                        typed.push(&callable);
                    }
                }
                typed
            }))
    })
    .map(|arr| arr.iter_shared().collect::<Vec<Callable>>())
    .unwrap_or_default();
```

**Change 3.4:** Update `is_valid` method (line ~125)

```rust
// BEFORE:
for (i, check) in self.validity_checks.iter().enumerate() {
    if !check.evaluate(agent_state, world_state) {  // Treating as PreconditionHandler
        log_debug!(
            "'{}' validity check {} failed (op: {:?}, property: '{}')",
            self.name,
            i,
            check.operation,
            check.property_name
        );
        return false;
    }
}

// AFTER:
for (i, check) in self.validity_checks.iter().enumerate() {
    let result = check.call(&[agent_state.to_variant(), world_state.to_variant()]);
    if !result.try_to::<bool>().unwrap_or(false) {
        log_debug!(
            "'{}' validity check {} failed",
            self.name,
            i
        );
        return false;
    }
}
```

**Why:** validity_checks are Callables, not PreconditionHandlers, so we need to invoke them.

**Verification:**
```bash
cd addons/GdPlanningAI/rust
cargo build --release
# Should compile without errors

# Copy the DLL to the correct location (Windows)
copy target\release\gdplanningai_rust.dll ..\bin\windows\

# Or .so file (Linux/macOS)
cp target/release/libgdplanningai_rust.so ../bin/linux/
```

---

## Verification & Testing

### Step 5: Run Tests

**5.1: Run all tests**

```bash
make test
```

Expected output:
```
✅ All 9/9 tests passing

test/integration/test_blackboard_clone.gd (3 tests)
test/integration/test_planning_engine.gd (4 tests)
test/unit/test_simple.gd (2 tests)
```

**5.2: If tests fail, check:**

1. **DLL/SO copied to correct location?**
   ```bash
   # Windows
   ls addons/GdPlanningAI/bin/windows/gdplanningai_rust.dll
   
   # Linux
   ls addons/GdPlanningAI/bin/linux/libgdplanningai_rust.so
   ```

2. **Godot imported resources?**
   ```bash
   godot --headless --import --quit
   ```

3. **GUT installed correctly?**
   ```bash
   ls addons/gut/gut_cmdln.gd
   ```


---

## Test Coverage Summary

After completing all fixes, you should have **9/9 tests passing**:

### Blackboard Tests (3 tests)
- `test_clone_creates_independent_copy` - Mutations to clones don't affect originals
- `test_clone_preserves_all_properties` - All data types clone correctly
- `test_multiple_clones_are_independent` - Multiple simultaneous clones work

### Planning Engine Tests (4 tests)
- `test_empty_plan_returns_failure` - No goals/actions returns failure
- `test_trivial_goal_already_satisfied` - Already-met goals return zero-cost plan
- `test_simple_one_action_plan` - Single-action plan generation
- `test_chooses_lower_cost_plan` - Cost optimization works correctly

### Basic Tests (2 tests)
- `test_basic_assertion` - GUT framework works
- `test_godot_types` - Godot type handling works

---

## Technical Background

### Why These Fixes Were Needed

**Type System Mismatch:**
- GDScript: Dynamically typed, arrays untyped by default
- Rust: Statically typed, expects `Array<T>` with explicit types
- Bridge: Must handle both typed and untyped variants gracefully

**Silent Failures:**
- Original code used `.unwrap_or_default()`, which silently created empty arrays on parse failure
- Empty arrays passed all validation (empty preconditions = satisfied)
- Tests passed but planning logic was broken

**Key Insight:**  
GDScript-Rust interop requires defensive parsing. Always provide fallback conversions for untyped variants (`VarArray`) when expecting typed arrays (`Array<T>`).

---

## Summary of Changes

### Files Created
- `addons.jsonc` - godot-env configuration for GUT
- `.gutconfig.json` - GUT test runner configuration
- `Makefile` - Convenient test commands

### Files Modified
- `.gitignore` - Fixed to track our addon, ignore third-party addons
- `addons/GdPlanningAI/rust/src/gdpai_blackboard.rs` - Added `#[func]` to `clone_for_simulation`
- `addons/GdPlanningAI/rust/src/goal.rs` - Added VarArray fallback for `desired_state`
- `addons/GdPlanningAI/rust/src/action.rs` - Fixed `validity_checks` type and parsing

### Quick Reference Commands

```bash
# Install GUT addon
godot-env addons install

# Build Rust extension
cd addons/GdPlanningAI/rust && cargo build --release

# Import Godot resources
godot --headless --import --quit

# Run all tests
make test

# Run specific test suites
make test-rust        # Rust unit tests only
make test-godot       # Godot integration tests only
make test-godot-verbose  # With detailed output
```

---

**Implementation Time:** ~2-3 hours (including debugging and verification)

**Expected Result:** ✅ 9/9 tests passing with full Rust-GDScript interop working correctly.
