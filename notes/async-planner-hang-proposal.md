# Async Planner Hang Proposal

**Date**: 2026-09-03 **Context**: Hunger fan-out causes ~2s main-thread stalls despite Rayon background planning

______________________________________________________________________

## Flowchart

```text
trigger (goal tick) -> submit_plan [scheduler.rs:238-264,494-507]
  -> Rayon worker: engine.rs:226-493 search loop
  -> yield? Pending(custom cb [simulation.rs, expander.rs]
    | iteration_budget [engine.rs:243-250]) : keep searching
  -> resume via gdpai_autoload _process -> process_callbacks
    -> scheduler.rs:205-209 -> complete -> agent resumes
```

Main thread sits idle while waiting (`_waiting_for_plan=true` [gdpai_agent.gd:74-80,148,177]); nothing else can progress until the worker finishes the burst.

______________________________________________________________________

## Why 2s Hangs Happen

- Eat -> Cook -> Dig x5 / GoTo x2 fan-out: 15110-16710 branches, 1783-2296ms per burst vs Maintain Fire 118 branches / ~100ms.
- Only yields are custom precond/cost/effect callbacks or every 20000 iterations -> `Pending(0)` [engine.rs:243-250]. Pure-symbolic bursts never yield, so one Rayon task stays CPU-busy.
- Main thread polls each frame but sees stale `pending_request_id` and spams `NOT resuming` [scheduler.rs:205-209]. Log spam is a symptom, not the cause.
- Defaults amplify it: `max_recursion 100`, `iteration_budget 20000` [gdpai_agent_config.gd].

______________________________________________________________________

## Fix Directions

### 1. Chunked / time-sliced search yielding Pending(0)

Yield every N expansions or M ms so `process_callbacks` can interleave and cancel. Smallest latency win, keeps completeness.

Tradeoff: more resume overhead; needs budget plumbing in `engine.rs`.

### 2. Branch-cap / fan-out reduction + visited pruning

Cap duplicate Dig/GoTo expansions, memoize visited state, prefer symbolic Requirements-Provisions over simulation. Attacks branch count directly (16k -> hundreds).

Tradeoff: risks pruning valid plans; needs domain tuning and tests.

### 3. Reduce main-thread callback cost

Batch custom callbacks, cache pure cost/precond results, move checks to builtin preconditions where possible.

Tradeoff: only helps callback-bound plans; does not fix pure-symbolic bursts like Hunger.

______________________________________________________________________

## Trap

Do not just silence `NOT resuming` logging. That hides the stall signal while the worker still blocks plan completion.

## Suggested Next Step

Prototype (1) with a small expansion/ms budget, then measure Hunger branch count before attempting (2).
