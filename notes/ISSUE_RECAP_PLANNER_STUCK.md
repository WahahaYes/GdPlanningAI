# Issue Recap: Planner Stuck on Initial State Requirements

**Date:** 2026-06-04
**Status:** Root Cause Identified

## Problem
The GOAP planner fails to find plans when a requirement for an action is already satisfied by the agent's initial state. This is most visible in `test_eat_alone_succeeds_when_already_holding_food`, where the agent is already holding a banana, but the planner returns an empty plan for the "Eat" action.

## Root Cause
In `engine.rs`, the planner's state machine works as follows:
1. **Searching Phase**: The planner finds candidate actions that satisfy open needs.
2. **Expansion**: When an action is added, its requirements are added to the branch's `open_requirements` at position 0.
3. **Transition to Verifying**: A branch only moves from `Searching` to `Verifying` if `open_requirements` and `open_preconditions` are **both empty**.

The check for whether the **initial state** satisfies requirements only happens in `process_simulation`, which is only called when the branch is in `Initializing`, `Rippling`, or `Verifying` states. 

Consequently, if an action introduces a requirement that is already met by the initial state:
- The `Searching` phase won't move to `Verifying` because `open_requirements` is not empty.
- The `Searching` phase won't find any new actions to satisfy the requirement (because it's already met).
- The branch becomes a "dead end" in the search, even though it's actually complete.

## Potential Fix
We need to ensure that requirements satisfied by the initial state are cleared during or immediately after the `Searching` expansion step.

### Proposed Changes in `addons/GdPlanningAI/rust/src/planner/engine.rs`:
In the `Searching` match arm of `step_search`, after prepending an action and adding its requirements:
1. Check each new requirement against `self.ctx.initial_provisions`.
2. If satisfied, do not add it to `open_requirements` (or remove it immediately).

This will allow the "all needs empty" check to pass and move the branch to the `Verifying` phase.

## Impact on Other Tests
This likely explains the timeouts in `test_campfire_example_smoke.gd` as well. If the planner is ignoring valid (but "incomplete" looking) branches, it may be over-exploring other suboptimal or deeper branches, leading to search space explosion and timeouts.
