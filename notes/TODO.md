# GdPlanningAI - Outstanding Work

This document tracks pending work items across the project. For completed work, see `completed/`.

---

## High Priority

### Requirements/Provisions Example Migration

**Document:** `PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md`

**Status:** Phase 4 complete, Phases 5-6 pending

**Remaining Items (Phase 5 - Migrate Examples):**
- Hunger food chain (Pickup → Eat) already migrated with requirements/provisions
- ShakeTreeAction cannot be fully migrated - object spawning effects cannot be modeled by requirements/provisions, will retain placeholder hunger gain
- Focus on compositional GoToAction prototype for remaining migration work

**Implementation Guide:** See `COMPOSITIONAL_GOTO_INTERACTION_PLAN.md` for detailed implementation steps

**Remaining Items (Phase 6 - Cleanup and Helper APIs):**
- Add convenience constructors/helpers for common requirement/provision patterns
- Add documentation for common patterns
- Evaluate whether existing precondition helpers should produce requirement/provision metadata automatically

**Benchmark Goal:** Replace `SpatialAction` with compositional `GoToAction + interaction` actions

**Context:** The requirements/provisions system is implemented and working for the hunger example, but needs broader adoption across examples and better ergonomics.

---

## Medium Priority

### Campfire Example Implementation

**Document:** `CAMPFIRE_EXAMPLE_PLAN.md`

**Status:** Not implemented

**Required Components:**
- `examples/behaviors/campfire/` - partially exists (has behavior config and eat action)
- `examples/objects/wood_pile/` - missing
- `examples/objects/campfire/` - missing
- `examples/objects/potato/` - missing
- `examples/shared/systems/potato_spawner/` - missing
- `examples/campfire_2d.tscn` - missing demo scene
- 2D prefabs for campfire, wood piles, potatoes, agents

**Concepts to Demonstrate:**
- Resource transformation (raw potato → cooked potato)
- Multi-step preparation chains (dig → cook → eat)
- Competing priorities (hunger vs fire maintenance)
- Dynamic spawning (potato respawns)
- Inventory management (single held_item slot)
- Task switching (drop held items when priorities change)
- Threshold-based actions (cooking requires fire fuel >= threshold)

**Context:** This is a major planned example that would demonstrate advanced planning patterns. The plan is detailed and ready for implementation.

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
- `completed/` - Completed implementation plans with no remaining work
- `reference/` - Reference and architecture documents (no action items)
- `bugs/` - Historical bug reports and fix plans
- `historical/` - Historical analysis and design exploration
- `superseded/` - Superseded implementation approaches
- `PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md` - Active plan (Phases 5-6 pending)
- `CAMPFIRE_EXAMPLE_PLAN.md` - Active plan (not implemented)
- `EXAMPLES_PLAN.md` - Active plan (partial implementation)
- `IMPLEMENTATION_PLAN.md` - Active plan (status unclear)

---

## Last Updated

2026-05-08
