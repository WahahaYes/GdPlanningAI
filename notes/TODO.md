# GdPlanningAI - Outstanding Work

This document tracks pending work items across the project. For completed work, see `completed/`.

---

## High Priority

### Phase 2: Modular Search Retry
**Document:** `PHASE_2_RETRY_LEARNINGS.md`
**Status:** Resetting after failed attempt.
**Key Goals:**
- Unify backward expansion and forward validation simulation logic.
- Robust state pruning (Visited Set) that ignores "noisy" simulation data.
- Scale heuristics and action costs to consistent magnitudes.
- Improved symbolic requirements for better branch pruning.

---

## Medium Priority

### Campfire Example Implementation

**Document:** `CAMPFIRE_EXAMPLE_PLAN.md` (updated 2026-05-10 for GoToAction architecture)

**Status:** Plan refreshed, ready for implementation

**Already exists:**
- `examples/behaviors/campfire/` — `CampfireBehaviorConfig`, `MaintainFireGoal`, `EatHeldFoodAction` (needs `GoToAction` added)
- `examples/behaviors/hunger/` — `HungerBehaviorConfig`, `HungerGoal`, `HungerPropertyUpdater`
- `examples/objects/holdable/` — `HoldableObject`, `PickupAction`, `DropItemAction`

**New files to create:**
- `examples/objects/wood_pile/` — `wood_pile_object.gd`, `pick_up_wood_action.gd`
- `examples/objects/campfire/` — `campfire_object.gd`, `add_fuel_action.gd`, `cook_potato_action.gd`
- `examples/objects/potato/` — `potato_object.gd`, `dig_potato_action.gd`
- `examples/shared/systems/potato_spawner/` — `potato_spawner.gd`
- `examples/campfire_2d.tscn` — demo scene
- 2D prefabs for campfire, wood piles, potatoes, agents

**Concepts to Demonstrate:**
- Resource transformation (raw potato → cooked potato)
- Multi-step preparation chains (dig → cook → eat)
- Competing priorities (hunger vs fire maintenance)
- Dynamic spawning (potato respawns)
- Inventory management (single held_item slot)
- Task switching (drop held items when priorities change)
- Threshold-based actions (cooking requires fire fuel >= threshold)
- GoToAction chaining pattern (navigation + interaction as separate actions)

**Estimated:** ~6-7 hours

---

### Examples Reorganization

**Document:** `EXAMPLES_PLAN.md`

**Status:** Partially implemented

**Completed:**
- `examples/behaviors/` directory exists with campfire/, hunger/, wander/
- `examples/objects/` directory exists with food/, fruit_tree/, holdable/
- `examples/shared/` directory exists with ui/

**Remaining:**
- Verify structure matches proposed `shared/behaviors/`, `shared/objects/`, `demo_2d/` layout
- Remove `Sample` prefixes from class names (if not already done)
- Update `examples/README.md` to document the new structure
- Remove obsolete threading demo scenes (if any remain)

**Context:** The examples directory has been partially reorganized but may need verification against the original plan.

---

## Low Priority / Status Unclear

### GDScript Migration Plan

**Document:** `IMPLEMENTATION_PLAN.md`

**Status:** Status unclear - may be superseded by async planner migration

**Potential Remaining Tasks:**
- Bug fixes in `GdPAILocationData` (backing variable recursion bug)
- Bug fixes in `GdPAIBehaviorConfig` (shared-resource bug)
- Precondition refactor (push serialization down, remove class_names)
- Bridge internalization (hide from users)
- Agent simplification (remove await, remove validity pre-filter)
- Config cleanup (add max_recursion)
- SpatialAction cleanup (deduplicate nav-agent lookup)

**Context:** This is an older GDScript-side migration plan. Many tasks may have been superseded by the async planner migration. Review needed to determine which tasks are still relevant.

---

## Completed Work (Archived)

The following documents have been moved to `completed/`:

- `ASYNC_PLANNER_MIGRATION_PLAN.md` - Async planner migration complete
- `BACKWARD_CHAINING_GOAP_PLANNER_PLAN.md` - Backward-chaining GOAP planner complete (Phases 1-7)
- `PLACEHOLDER_AND_STATE_DEPENDENCY_CHAINING.md` - Resolved by backward planner implementation
- `THREADING_PLAN.md` - Superseded by async planner migration

---

## Reference Documents (No Action Items)

The following documents have been moved to `reference/` as reference/architecture material:

- `ARCHITECTURE_DESIGN.md` - Target GDScript architecture design
- `MIGRATION_OVERVIEW.md` - Context for why GDScript migration was needed
- `RUST_TEST_COVERAGE.md` - Rust test coverage analysis

These documents provide context and design guidance but contain no actionable work items.

---

## Bug Reports (Historical)

The following documents have been moved to `bugs/` as historical bug reports:

- `forward_state_accumulation_bug.md` - Forward state accumulation bug report
- `forward_state_accumulation_plan.md` - Fix plan for forward state accumulation
- `gdformat_lambda_bug_issue.md` - GDScript lambda formatting bug
- `hunger_example_planner_limitation.md` - Hunger example planner limitation
- `hunger_goal_backward_chaining_incompatibility.md` - Backward chaining incompatibility
- `planner_custom_precondition_bug.md` - Custom precondition bug

These are historical bug reports. Status unclear - may be resolved by recent planner changes.

---

## Historical Documents

The following documents have been moved to `historical/` as historical analysis or implementation notes:

- `INTEGRATION_TEST_CHANGES.md` - Integration test setup guide
- `PRECONDITIONS_REQUIREMENTS_PROVISIONS_API_SKETCH.md` - Early API sketch for requirements/provisions
- `requirements_provisions_necessity_analysis.md` - Analysis of requirements/provisions necessity

These documents capture historical analysis and early design exploration.

---

## Superseded Documents

The following documents have been moved to `superseded/` as superseded implementation approaches:

- `OPTION_C_IMPL.md` - Rust channel bridge implementation (not chosen; async planner used instead)
- `EXAMPLES_REORGANIZATION.md` - Moving examples to top-level (alternative approach not taken)

These documents describe implementation approaches that were considered but not pursued.

---

## Document Organization Summary

The `notes/` directory is now organized as follows:

- `TODO.md` - This file, tracking outstanding work
- `GOTOACTION_PORT_TODO.md` - Detailed TODO for GoToAction pattern migration
- `completed/` - Completed implementation plans with no remaining work
- `reference/` - Reference and architecture documents (no action items)
- `bugs/` - Historical bug reports and fix plans
- `historical/` - Historical analysis and design exploration
- `superseded/` - Superseded implementation approaches
- `PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md` - Active plan (Phase 5 in progress)
- `COMPOSITIONAL_GOTO_INTERACTION_PLAN.md` - Detailed design for compositional GoTo approach (superseded by GOTOACTION_PORT_TODO.md)
- `CAMPFIRE_EXAMPLE_PLAN.md` - Active plan (not implemented)
- `EXAMPLES_PLAN.md` - Active plan (partial implementation)
- `IMPLEMENTATION_PLAN.md` - Active plan (status unclear)
---

## Last Updated

2026-05-09
