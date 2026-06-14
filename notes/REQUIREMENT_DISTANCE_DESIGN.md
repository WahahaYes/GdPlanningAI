# Multi-Requirement Action Chaining Design

## Problem

Actions with multiple requirements that need different predecessor types cannot be chained.

Example: `CookPotatoAction` requires:
1. `binding_equals("held_item", "potato")` — satisfied by `DigPotatoAction`
2. `fact("at_target", [campfire_location])` — satisfied by `GoToAction`

Current rule: a predecessor's provisions can only satisfy requirements at `pos == 0`. When a predecessor is prepended, existing unsatisfied requirements shift from `pos=0 → pos=1`. Once shifted, **no action can ever satisfy them**.

This means whichever predecessor is selected first (`DigPotato` or `GoTo`), the other requirement gets buried at `pos>0`. The branch eventually fails terminal validation because `open_requirements` is non-empty.

## Root Cause

The `pos` field encodes both:
1. **Distance** — how many actions away the requirement is from the current head
2. **Eligibility** — only requirements at `pos==0` can be satisfied by the next predecessor

These two concerns are conflated into one number.

## Proposed Fix: Separate Distance from Eligibility

### Option A: Satisfy Any Unsatisfied Requirement (Recommended)

Remove the `pos == 0` check entirely. A predecessor can satisfy **any** unsatisfied requirement in `open_requirements`, regardless of distance.

**Pros:**
- Natural — doesn't matter if `GoTo` is added before or after `DigPotato`; whichever is selected next can satisfy whichever requirement it provides
- Backward chaining just needs all requirements to be satisfied by *some* predecessor, order of predecessors doesn't constrain which requirement they satisfy

**Cons:**
- Breaks the "sequential dependency" semantic where `pos` encoded action distance
- Could allow a predecessor to satisfy a requirement that conceptually belongs to a different branch of the chain

**Mitigation:** Keep `pos` for distance tracking and debug display, but don't use it for eligibility filtering.

### Option B: Per-Action Requirement Slots

Each action in the chain "claims" specific requirements it introduces. Predecessors only see the requirements of the immediate successor (the action at `pos=0`), but can also see "orphaned" requirements from deeper in the chain that haven't been claimed yet.

More complex — adds slot tracking overhead.

### Option C: Merge Spatial Requirements into Preconditions

Convert `at_target` from a `RequirementSpec` into a `PreconditionSpec`. Spatial requirements are fundamentally state-based (agent position), not binding-based.

**Pros:**
- `GoTo` action satisfies it via simulation effect, not symbolic provision
- Removes the multi-requirement problem entirely for spatial navigation

**Cons:**
- `at_target` currently carries a `GdPAILocationData` binding that gets injected into `GoToAction.target_location` — moving this to preconditions loses the binding mechanism
- Requires redesigning how `GoToAction` discovers its target

## Recommendation

**Option A** with `pos` kept for distance/bookkeeping but ignored for eligibility.

### Code Changes

1. **`expander.rs`**: Remove `if *pos != 0 { continue; }` guards in `find_candidates`
2. **`engine.rs`**: When prepending a predecessor, only shift `pos` for requirements that were **not** satisfied by the new action (currently shifts all)
3. **`engine.rs` forward validation**: Check requirements at their actual `pos`, not just `pos == simulation_index`

### Example Walkthrough

Goal: `Eat Held Food` (requires `held_item`, `is_food`)

**Before (broken):**
1. Prepend `Cook Potato` → satisfies `is_food`, leaves `held_item` at `pos=1`
2. Try prepend `Dig Potato` → `held_item` is at `pos=1`, can't satisfy → branch invalid
3. Try prepend `GoTo` → no `at_target` at `pos=0` → no-op → branch invalid

**After (fixed):**
1. Prepend `Cook Potato` → satisfies `is_food`, leaves `held_item` at `pos=1`
2. Prepend `Dig Potato` → `held_item` is at `pos=1`, but we now allow satisfying any unsatisfied req → `held_item` satisfied, leaves `at_target` at `pos=2`
3. Prepend `GoTo` → `at_target` at `pos=2`, allowed to satisfy → all needs met

## Edge Cases

- **Multiple `at_target` requirements**: `GoTo` wildcard should be able to satisfy any/all of them. The binding injection mechanism already handles this via `inject_binding`.
- **Order of predecessors**: `GoTo` can be prepended before or after `Dig Potato`; both orders should work because each predecessor independently satisfies the requirements it can.
- **Same requirement from multiple depths**: Greedy clearing already handles this — identical requirements are deduplicated when a provision matches.

## Files to Modify

- `addons/GdPlanningAI/rust/src/planner/expander.rs` — `find_candidates` eligibility check
- `addons/GdPlanningAI/rust/src/planner/engine.rs` — position shifting logic during branch expansion
- `addons/GdPlanningAI/rust/src/planner/engine.rs` — forward validation check

---

## Investigation Log (Session of 2026-06-14)

### Status: Option A Step 1 Complete, Steps 2–3 Pending

**What was done:**
- Removed `if *pos != 0 { continue; }` guards in `expander.rs` `find_candidates`.
- Fixed unused-variable warnings (`pos` → `_pos`).
- Rust unit tests pass.

**What still fails:**
- Godot integration test `test_campfire_example_smoke.gd` times out on all campfire-related tests.

### Critical Discovery: `Cook Potato` and `Dig Potato` Are Invisible to the Planner

The timeout debug tree shows the planner stuck in useless loops (`Pick Up Wood` → `Eat Held Food` → `Pick Up Wood`…), but a **grep of the entire `test_output.log` for "Cook" or "Dig" returns zero hits**. These two actions never appear in the planner search tree at all.

**Evidence:**
- Scheduler log confirms `actions_count=15` — the expected count including campfire (2) + wood piles (4) + potatoes (5) + agent self-actions (4).
- `Add Fuel` (from the same `CampfireObject`) **does** appear in the tree.
- `Pick Up Wood` (from `WoodPile` objects) **does** appear in the tree.

**Key difference:**
- `Add Fuel` has **no validity checks** (`get_validity_checks()` returns `[]`).
- `CookPotatoAction` has 3 custom validity checks (`check_is_object_valid(campfire_ref)`, `check_is_object_valid(object_location)`, `check_is_object_valid(interactable_attribs)`).
- `DigPotatoAction` has 4 custom validity checks (same pattern plus `potato_ref.entity`).
- `Pick Up Wood` has 3 custom validity checks (`check_is_object_valid(holdable_item)`, `check_is_object_valid(object_location)`, `check_is_object_valid(interactable_attribs)`) — yet it **does** appear.

### Why Do Custom Validity Checks Block These Actions?

Custom preconditions (`PreconditionCustomWithDeps`) are evaluated via async Godot callbacks:

1. `find_candidates` iterates over candidate actions.
2. For each custom validity check, `eval_precondition` returns `StepResult::Pending(request_id)`.
3. `find_candidates` sets `validity_failed = true` and skips the action (`continue`).
4. The engine parks the node and waits for the callback response.
5. When resumed, `find_candidates` checks the cache. If cached, it proceeds to the next check.
6. Each action needs N resumptions to clear N custom validity checks.

**Why `Pick Up Wood` appears but `Cook Potato` / `Dig Potato` do not:**
Hypothesis 1: The search explores `Pick Up Wood` branches first (lower cost), gets stuck in infinite loops, and the A* queue becomes saturated with useless branches before ever reaching the branches that would contain `Cook Potato` or `Dig Potato`.

Hypothesis 2: The `get_action_cost()` or `simulate_effect()` of `CookPotatoAction` / `DigPotatoAction` returns `INF` / fails during discovery, causing `get_discovery_result` to return `StepResult::Invalid`. This would silently drop the action without adding it to the tree. However, `AddFuelAction` uses the same `campfire_ref` and the same `world_state.get_object_for(campfire_ref)` call, and its discovery succeeds — making this less likely.

Hypothesis 3: The `check_is_object_valid` callback returns `false` for `CookPotatoAction` / `DigPotatoAction` because one of their dependent objects (e.g., `potato_ref.entity` for `DigPotatoAction`) is not valid at planning time. The `PotatoSpawner` uses `call_deferred("add_child", potato)`, so potatoes are added to the scene tree asynchronously. The test pumps 10+3 frames before planning, but there may be a subtle timing issue where the potatoes are not fully initialized when the world state is captured. However, `actions_count=15` implies the potatoes ARE discovered by `_collect_worldly_actions()`.

### The Real Infinite Loop

Even if `Cook Potato` / `Dig Potato` were visible, the planner would still get stuck because of the **remaining `pos` shifting bug** (Option A steps 2–3 not yet implemented):

- `Eat Held Food` requires `is_food` and `exists(held_item)`.
- `Pick Up Wood` satisfies `exists(held_item)` but reintroduces `at_target(wood_loc)`.
- `Eat Held Food` satisfies `Pick Up Wood`'s precondition `held_item == ""` (via `simulate_effect` setting `held_item = ""`).
- So the planner cycles: `Pick Up Wood` → `Eat Held Food` → `Pick Up Wood` → `Eat Held Food` …
- Each cycle adds another `at_target` requirement.
- The fingerprint-based cycle detection (`visited` set) does NOT catch this because each new `at_target` makes the `open_requirements` unique.

### Next Steps for a New Agent

**Priority 1: Determine why `Cook Potato` / `Dig Potato` are invisible.**
- Add targeted `log_warn!` or `log_debug!` in `find_candidates` (around the validity-check loop and discovery-result handling) to print which actions are being skipped and why.
- Temporarily remove `get_validity_checks()` from `CookPotatoAction` and `DigPotatoAction` (return `[]`) to see if they then appear in the tree. If yes, the issue is the async validity-check mechanism.
- If they still don't appear after removing validity checks, the issue is in discovery (`get_action_cost` returning INF or `simulate_effect` failing).

**Priority 2: Implement Option A steps 2–3 in `engine.rs`.**
- Step 2: In `expand` (around line 326-331), only shift `pos` for requirements that were **not** satisfied by the newly prepended action. Currently ALL requirements shift.
- Step 3: In `process_simulation` forward validation, check requirements at their actual `pos`, not just `pos == simulation_index`.

**Priority 3: Add a cycle-detection guard for accumulating `at_target` requirements.**
- The `visited` fingerprint does not catch `Pick Up Wood` → `Eat Held Food` loops because each iteration adds a new `at_target`.
- Consider adding a max-depth or max-requirement-count limit to prevent runaway accumulation of identical-type requirements.

### Intro Message for New Agent Context

> We are fixing campfire test timeouts in the GdPlanningAI planner. Option A step 1 (removing `pos==0` eligibility checks in `expander.rs`) is done. The planner is still timing out. The debug tree reveals the planner is stuck in `Pick Up Wood` → `Eat Held Food` loops, accumulating `at_target` requirements. Critically, `Cook Potato` and `Dig Potato` NEVER appear in the search tree — a grep of the full log finds zero hits. These actions both have custom `check_is_object_valid` validity checks that require async Godot callbacks, while `Add Fuel` (same object, no validity checks) does appear. Your first task is to determine WHY `Cook Potato` and `Dig Potato` are invisible — add logging or temporarily remove their validity checks to isolate the cause. Then implement Option A steps 2–3 in `engine.rs` (pos-shifting and forward-validation fixes) to stop the infinite `at_target` accumulation loops.
