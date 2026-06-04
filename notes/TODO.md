# GdPlanningAI - Outstanding Work

This document tracks pending work items across the project. For completed work, see `completed/`.

---

## High Priority

### Backward-Chaining Planner Enhancements
The core backward-chaining GOAP planner is functional. Next steps focus on robustness and edge cases.

**Current Focus:**
- **Action Binding Stability:** Ensure action bindings (e.g., target objects) are consistently handled across repeated occurrences of the same action in a chain.
- **State Pruning (Visited Set):** Implement robust state pruning that ignores "noisy" simulation data (e.g., precise hunger/fuel values) and focuses on discrete state changes.
- **Heuristic Scaling:** Ensure heuristics and action costs are scaled to consistent magnitudes for optimal A* search.
- **Performance:** Optimize the transition from backward search to forward validation.

---

## Medium Priority

### Examples & Documentation
- **3D Campfire Demo:** Verify and polish the 3D version of the campfire demo.
- **API Documentation:** Update the core API documentation (especially around Requirements/Provisions) to match the implemented Rust-side logic.
- **User Guide:** Create a "Getting Started" guide for the new GoToAction architecture.

---

## Low Priority

### Framework Cleanup
- **SpatialAction Removal:** Verify all references to the legacy `SpatialAction` are removed or migrated to `GoToAction` + interaction pattern.
- **Config Cleanup:** Consolidate agent configuration properties and ensure `max_recursion` is respected everywhere.
- **Bridge Internalization:** Continue hiding bridge implementation details from the end user.

---

## Recently Completed (Archived)

The following documents have been moved to `completed/`:

- `CAMPFIRE_EXAMPLE_PLAN.md` - Campfire example implemented (both 2D and 3D)
- `EXAMPLES_PLAN.md` - Examples directory reorganized and structure documented
- `PRECONDITIONS_REQUIREMENTS_PROVISIONS_PLAN.md` - Phase 1-6 complete (Requirements/Provisions + GoToAction composition)
- `PLANNER_STRATEGY_MODULARIZATION_PLAN.md` - Planner engine modularized into engine/expander/controller/policy
- `ASYNC_PLANNER_MIGRATION_PLAN.md` - Async planner migration complete
- `BACKWARD_CHAINING_GOAP_PLANNER_PLAN.md` - Core backward-chaining logic implemented
- `GOTOACTION_PORT_TODO.md` - Migration from SpatialAction to GoToAction pattern complete

---

## Superseded Documents

The following documents have been moved to `superseded/`:

- `ASYNC_CALLBACK_ARCHITECTURE_PLAN.md` - Failed two-phase async implementation attempt
- `COMPOSITIONAL_GOTO_INTERACTION_PLAN.md` - Superseded by GoToAction pattern implementation
- `IMPLEMENTATION_PLAN.md` - Older GDScript-focused migration plan
- `OPTION_C_IMPL.md` - Rust channel bridge implementation (not chosen)

---

## Historical & Reference

- `historical/` - Contains retrospectives and status reports from previous phases.
- `reference/` - Contains architectural design and design revision notes.
- `bugs/` - Historical bug reports (many now resolved by the backward planner).

---

## Last Updated

2026-06-04
