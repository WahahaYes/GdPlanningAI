# Integration Test Failures Investigation

This document tracks and analyzes the nature of current integration test failures in `GdPlanningAI`.

## 1. Optimal Path Selection (A*)
**Test**: `test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion`  
**File**: `res://test/integration/test_async_planner.gd`

### Symptoms
The planner correctly finds a successful plan but returns an expensive one (cost 10.0) instead of the optimal multi-step plan (cost 2.0).

### Search Tree Analysis
```text
Goal: 'ReadyCampfireMeal' (reward=100.0) pre=[Agent.has_food == Bool(true), Agent.has_fire == Bool(true)]
└── ROOT
  open_pre=[Agent.has_food == Bool(true), Agent.has_fire == Bool(true)] open_req=[]
    ├── Try 'ExpensiveDirectCampfirePrep' (cost=+10.00, total=10.00)
      satisfies_pre=[Agent.has_food == Bool(true), Agent.has_fire == Bool(true)] satisfies_req=[]
    ├── Try 'GetFood' (cost=+1.00, total=1.00)
      satisfies_pre=[Agent.has_food == Bool(true)] satisfies_req=[]
      open_pre=[Agent.has_fire == Bool(true)] open_req=[]
    └── Try 'LightFire' (cost=+1.00, total=1.00)
      satisfies_pre=[Agent.has_fire == Bool(true)] satisfies_req=[]
      open_pre=[Agent.has_food == Bool(true)] open_req=[]
  RESULT: SUCCESS — [ExpensiveDirectCampfirePrep] cost=10.00
```

### Potential Causes
- **Termination Logic**: The planner might be stopping at the first valid plan found (`TerminationStrategy::FirstComplete`) even when configured for `BestCost`.
- **Search Order**: In A*, a node with cost 1.0 (`GetFood`) should be expanded before a node with cost 10.0. The tree shows they were all generated as candidates, but it appears `ExpensiveDirectCampfirePrep` was processed through to `Verifying` and completion before the others were expanded.
- **Async Yielding**: If `GetFood` returned `Pending` while `ExpensiveDirectCampfirePrep` was `Ready`, the engine might have completed the ready one and exited before the pending one resumed.

---

## 2. Campfire Scenario Timeouts
**Tests**: `test_full_cooking_chain`, `test_preemptive_fire_maintenance`, etc.  
**File**: `res://test/integration/test_campfire_example_smoke.gd`

### Status
**FIXED** (Timeouts resolved, most tests passing)

### Symptoms
Previously, these tests timed out waiting for a plan. Now they complete, and 4 out of 6 tests in this suite pass.

### Analysis
- **Fixed Timeout**: The "Strict Enrollment" architecture significantly pruned the search space by rejecting invalid `BindingInSet` candidates (like picking up Wood for Food requirements). This reduced the state explosion enough to allow the planner to finish within the iteration budget even for complex 15+ action scenarios.
- **Improved Plan Quality**: Tests like `test_preemptive_fire_maintenance` and `test_fire_too_low_to_cook` are now passing correctly because the planner is no longer distracted by thousands of invalid symbolic branches.

### Remaining Issue
- `test_full_cooking_chain` still fails because it returns a shorter plan than expected (3 actions instead of 5) and causes an index error in the test script. This is now a "Plan Quality" issue rather than a performance/timeout issue.

---

## 3. `BindingInSet` Validation Bypass
**Test**: `test_binding_in_set_requires_world_group_membership`  
**File**: `res://test/integration/test_requirements_provisions.gd`

### Status
**FIXED**

### Symptoms
The planner chooses `PickupRock` (cost 0.5) to satisfy a requirement for `held_item in edible`. Rocks are not in the `edible` group.

### Analysis
- **Optimistic Discovery**: `requirement.rs` intentionally treats `BindingInSet` as satisfied during discovery if the binding names match, assuming the "ripple" phase will catch it.
- **Verification Failure**: The forward simulation (`Verifying` state) was not re-validating the `BindingInSet` requirement against the concrete bound value.

### Fix
- Implemented `pending_req_validations` in `PlanBranch`.
- Every optimistic causal link (Action Provision -> Requirement) is recorded during expansion.
- During the grounded simulation (Rippling/Verifying phase), every recorded requirement is strictly validated against the actual world state before its consumer action is simulated.
- If validation fails, the branch is discarded.

---

## 4. Missing Candidate Discovery
**Test**: `test_search_returns_cheapest_valid_requirement_chain`  
**File**: `res://test/integration/test_requirements_provisions.gd`

### Symptoms
`RESULT: FAILURE — no plan found`. The tree only shows ROOT.

### Analysis
The planner is failing to find *any* actions that satisfy the goal `task_done == true`.
- **Hybrid Discovery Mismatch**: The `UseTool` actions in this test set `task_done` via an `effect_callable`. However, the discovery phase (`find_candidates`) only qualifies actions if they satisfy a need when simulated against the **InitialState**.
- **The Catch-22**: `UseTool` only sets `task_done` if `tool` is present. In the `InitialState`, the tool is missing. Therefore, the discovery simulation of `UseTool` shows no effect on `task_done`, and the action is never added to the frontier.
- **Solution**: Actions that satisfy goals via complex simulation must either:
    1. Advertise that effect via a symbolic `ProvisionSpec` (e.g., `ProvisionSpec::Fact("task_done", [])`).
    2. The planner needs a way to discover actions that *could* satisfy a precondition if their own requirements were met (which is currently what the symbolic layer is for).

---

## 5. Unexpected Success (Depth/Constraint Failure)
**Test**: `test_competing_priorities_hunger_wins`  
**File**: `res://test/integration/test_campfire_example_smoke.gd`

### Symptoms
`Agent should not plan an over-depth critical recovery`. The test expects failure but gets a 3-action plan.

### Analysis
This suggests the `max_depth` parameter passed from Godot isn't being strictly enforced, or the test's expectation of what constitutes "over-depth" doesn't match the planner's internal depth tracking (which now counts backward steps).
