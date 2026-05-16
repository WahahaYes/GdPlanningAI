# Campfire Interactive Planner Non-Termination

## Context

The campfire interactive scene was reduced to one agent to make runtime planning easier to inspect. The agent uses both hunger and campfire behavior configs, and the world includes potato, wood pile, campfire, and generic navigation actions.

The planner can find valid-looking campfire hunger chains in debug output, but the interactive scene does not receive a non-empty plan even after allowing the planner to run for minutes.

## Current Symptom

The agent repeatedly submits plans and receives empty successful plans while `Maintain Fire` is already satisfied. Once hunger becomes high enough for the hunger goal to be actionable, the planner begins exploring non-empty chains but does not appear to terminate and publish a result.

A representative log snippet shows search continuing after a best valid branch exists:

```text
[GdPAI | debug] Max depth 6 reached
[GdPAI | debug] Trying action 'Add Fuel' at depth 6 (cost 1.00)
[GdPAI | debug] Builtin precondition failed: "held_item" Equal Some(Str("wood")) (actual=None, target=Agent)
[GdPAI | debug] Max depth 6 reached
[GdPAI | debug] Trying action 'Eat Held Food' at depth 6 (cost 1.50)
[GdPAI | debug] Max depth 6 reached
[GdPAI | debug] Trying action 'Go To' at depth 6 (cost 207.68)
[GdPAI | debug] Pruning branch at depth 7 with estimated cost 424.82 (best valid: 306.07)
[GdPAI | debug] Trying action 'Go To' at depth 6 (cost 207.68)
[GdPAI | debug] Pruning branch at depth 7 with estimated cost 424.82 (best valid: 306.07)
[GdPAI | debug] Trying action 'Go To' at depth 6 (cost 254.95)
[GdPAI | debug] Pruning branch at depth 7 with estimated cost 472.08 (best valid: 306.07)
[GdPAI | debug] Trying action 'Go To' at depth 6 (cost 254.95)
[GdPAI | debug] Pruning branch at depth 7 with estimated cost 472.08 (best valid: 306.07)
[GdPAI | debug] Trying action 'Go To' at depth 5 (cost 254.95)
[GdPAI | debug] Finding candidates among 15 actions for 1 preconditions and 2 requirements
```

## What We Verified

The optimistic unbound hunger restore changed candidate discovery as expected. Once hunger crosses the current `HungerGoal` improvement threshold, logs show:

```text
Action 'Eat Held Food' satisfies open preconditions via effect
```

Debug output also showed forward-validated non-empty chains, including a chain shaped like:

```text
Drop Item
Go To
Dig Potato
Go To
Cook Potato
Eat Held Food
```

Observed valid branch costs included approximately:

```text
428.66
294.39
242.38
306.07
```

So the problem is no longer simply "no chain can be found." The planner can find valid chains, but it continues expanding/pruning instead of closing the search and publishing a result in the interactive scene.

## Current Intuition

The repeated `Go To` actions are likely informative. The planner action chain stores action indices, and action-specific wildcard bindings are currently keyed by action index:

```text
action_bindings: Vec<(action_index, fact_name, object_ids)>
```

This is problematic for generic reusable actions such as `GoToAction`, because a single action index can appear multiple times in one plan with different target locations:

```text
Go To potato
Dig Potato
Go To campfire
Cook Potato
Eat Held Food
```

If both `Go To` occurrences share the same action index, then occurrence-specific target bindings cannot be represented correctly by `action_index` alone. Every occurrence may see every binding for that action index. This can corrupt cost calculation, forward validation, execution binding injection, or all three.

There is a related scene/config issue: both hunger and campfire behavior configs add a generic `GoToAction`, so the action list may contain duplicate equivalent generic navigation actions. That can double candidate count and make repeated `Go To` expansion worse, but it is probably not the root bug. The deeper issue is that even one generic `GoToAction` can occur multiple times in a valid plan and needs occurrence-scoped binding.

## Suspected Root Cause

The planner represents selected actions by original action index, but wildcard/fact bindings are attached to action index rather than to a specific occurrence in the planned chain.

This is insufficient for reusable generic actions. It can cause:

- Duplicate bindings applied to the same action during validation.
- Ambiguous `GoToAction` target injection during GDScript deserialization.
- Multiple `Go To` branches that look distinct in search but collapse to the same action identity.
- Failure to converge or excessive expansion after finding a valid branch.

## Likely Fix Direction

Use occurrence-scoped bindings instead of action-index-scoped bindings.

Possible approaches:

1. Store action chain entries as occurrence records rather than bare action indices.
2. Assign an occurrence id each time an action is inserted into a branch.
3. Store bindings against occurrence id or action-chain position.
4. Serialize enough occurrence information to GDScript so repeated action indices can be duplicated or rebound independently.
5. During deserialization, duplicate action instances for repeated occurrence-sensitive actions, or otherwise ensure each planned occurrence has independent binding/runtime state.

A minimal serialization shape might be:

```text
action_chain: [action_index, action_index, ...]
action_bindings: [(chain_position, fact_name, object_ids)]
```

or:

```text
action_occurrences: [{ action_index, bindings }, ...]
```

The first option is less invasive but requires careful chain-position stability after insertion.

## Open Questions

- Should all repeated actions be duplicated for execution, or only actions with injected bindings/state?
- Should planner search forbid selecting the exact same action occurrence for the same unresolved requirement more than once?
- Should generic `GoToAction` be added by only one behavior config to reduce duplicate candidates?

This is a non-issue because only one behavior config is used per agent.

- Is exhaustive optimal search still too expensive after occurrence binding is fixed, or is most of the non-termination caused by binding ambiguity and duplicate navigation candidates?

## Next Investigation Steps

1. Inspect `PlanBranch.action_chain` and `action_bindings` usage end-to-end.
2. Confirm whether repeated `GoToAction` entries produce multiple bindings with the same action index.
3. Patch planner/result serialization to bind by occurrence position or occurrence id.
4. Update `GdPAIRustBridge.deserialize_plan_result()` to inject bindings per planned occurrence, not per original action object.
5. Re-run the campfire interactive capture with one agent.
