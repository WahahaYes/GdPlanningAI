# Async Callback Architecture Plan (Failed Implementation)

## Current State and Problems

The planner uses synchronous blocking callbacks from Rayon worker threads to the main Godot thread. Planner threads block waiting for GDScript callable responses, causing:

- **Test harness timeouts:** Tests must artificially pump callbacks via `process_callbacks()`, but planner threads still spend most time blocked
- **Production risk:** Current timeout-based workaround (1000ms with safe defaults) is not production-ready
- **Architectural bottleneck:** Synchronous blocking from worker threads to main thread limits parallelism

### Blocking Points

The planner has three blocking points in the simulation layer:
1. Custom precondition evaluation
2. Action cost calculation
3. Effect simulation

Each blocking point uses a channel receive operation that waits for the main thread to execute a GDScript callable and return a response.

## Architectural Options Considered

### Option 1: Branch State Tracking with Blocked/Pending States

Add a state to search nodes indicating if they're blocked on callbacks. Modify controllers to skip blocked nodes during pop and resume them when callbacks arrive.

**Pros:** Minimal changes to existing architecture, clear separation of blocked vs ready branches, can process multiple branches in parallel while waiting for callbacks.

**Cons:** Requires storing continuation state for each blocked branch, memory overhead for many blocked branches, still needs mechanism to resume branches when callbacks arrive, only partial solution that doesn't eliminate blocking.

### Option 2: Future-Based Async Callback Architecture

Replace blocking channels with Rust futures using async/await to make callback operations non-blocking.

**Pros:** True async/await semantics, no thread blocking, standard Rust async patterns.

**Cons:** Requires async runtime (tokio/async-std), major refactoring of planner engine where all functions become async, Rayon is designed for CPU-bound parallel work not async I/O, Godot's GDScript callables are synchronous and would need to be wrapped in blocking tasks which defeats the purpose.

**Verdict:** Not viable given current Godot integration constraints and Rayon architecture.

### Option 3: Two-Phase Expansion with Callback Continuation

Split expansion into two phases. Phase 1: Identify actions and send callback requests without blocking. Phase 2: When callbacks arrive, resume expansion with results.

**Pros:** Production-ready with true non-blocking architecture without timeouts, Rayon-compatible keeping existing thread pool architecture, Godot-compatible with no changes to GDScript callable integration, test-friendly by eliminating callback pumping bottleneck, can convert blocking calls gradually.

**Cons:** Complex state management tracking pending callbacks per branch, need bidirectional channels between scheduler and planner, requires unique request IDs to match callbacks to branches, significant refactoring of expansion logic.

**Recommendation:** Option 3 was selected for implementation.

## Implementation Attempt

### Infrastructure Completed

The following foundational infrastructure was successfully implemented:

1. **Notification Channel in scheduler**
   - Added `PlannerCallback` enum with `PreconditionResult`, `CostResult`, and `EffectResult` variants
   - Added `planner_callback_tx` channel to `ActiveJobHandle`
   - Implemented load spreading with configurable max callbacks per frame to prevent frame hitches

2. **Pending Expansion Types in planner types**
   - Added `PendingExpansion` struct to track expansions waiting on callbacks
   - Added `PendingCallback` enum with `Precondition`, `Cost`, and `Effect` variants
   - Added `ExpansionContinuation` enum to track what step to resume after callback
   - Added `try_recv` method to `PendingCallback` for non-blocking callback retrieval

3. **Engine Modifications**
   - Added `pending_expansions` field to `PlannerEngine`
   - Implemented `resume_pending_expansions` method with callback-to-expansion matching
   - Added continuation logic for sending next-phase requests (e.g., effect after cost)

4. **Simulation Result Enums**
   - `PreconditionResult` enum with `Ready(bool)` and `Pending(Receiver)` variants
   - `SimulationResult` enum with `Ready`, `PendingCost`, and `PendingEffect` variants
   - Added transitional `.block()` methods for backward compatibility

### Implementation Progress

1. **Removed blocking methods** - Removed `.block()` methods from `PreconditionResult` and `SimulationResult`

2. **Updated discovery and ripple** - Modified expander to return `PendingExpansion` when encountering pending results in ripple phase, skip candidates with pending results in discovery phase, store continuation data in `PendingCallback`

3. **Updated planner engine** - Modified to return `PlannerRunResult::Pending` when there are pending expansions, generate unique request IDs for pending callbacks, add request ID assignment in `resume_pending_expansions`

4. **Updated scheduler** - Modified to handle `PlannerRunResult` enum, currently treats `Pending` as failure as a temporary measure

5. **Updated integration tests** - Converted Rust tests to use builtin preconditions only to avoid async complexity

6. **Built release binary** - Successfully built

## Issues and Bugs Encountered

### Core Architectural Issue: Callback Routing Mismatch

The fundamental problem that prevented the implementation from working end-to-end was a mismatch in callback routing:

**Problem:** `PendingExpansions` stored their own receivers, but `resume_pending_expansions` checked the global callback channel. This created a situation where callbacks couldn't be matched to the specific pending expansions waiting for them.

**Root Cause:** The design mixed two different callback routing approaches:
- Receiver-based routing: Each pending expansion stored its own channel receiver
- Channel-based routing: The scheduler sent notifications to a global planner callback channel

These two approaches were incompatible. When a callback arrived via the global channel, there was no way to determine which specific pending expansion (and which receiver) should receive it.

### Planned Solution: Request ID-Based Routing

The solution was to refactor to use request_id-based routing instead of receiver-based routing:

1. Remove receivers from `PendingCallback` variants
2. Add `request_id: usize` to all `PendingCallback` variants
3. Add request ID to `CallbackRequest` and generate unique IDs
4. Update scheduler to include request ID in notifications
5. Implement request ID-based callback matching in the engine
6. Implement full continuation logic for all callback types

### Incomplete Resumption Logic

Even with request ID-based routing, the resumption logic was incomplete. The `resume_pending_expansions` method matched callbacks by request ID but didn't actually resume the expansion with the callback result. The following logic was never implemented:

1. **Cost callback resumption:** When cost callback arrives, use the cost value, request effect simulation with the same agent/world state, update pending expansion to wait for effect result

2. **Effect callback resumption:** When effect callback arrives, use the updated agent/world snapshots, apply the effect to the branch state, add the action to the action chain, push the updated branch to the search controller for further expansion

3. **Precondition callback resumption:** When precondition callback arrives, use the boolean result, if satisfied continue with cost/effect simulation, if not satisfied discard the branch

4. **Scheduler integration:** Remove the temporary "treat Pending as failure" logic, implement proper job pausing/resumption in scheduler, store pending expansions per job, resume planning when callbacks arrive

### Test Suite Complexity

The test suite needed significant updates to support the async architecture:

- Mock responder needed to send `PlannerCallback` notifications
- Integration tests needed to test async behavior specifically
- Tests needed to verify request ID uniqueness
- Tests needed to verify pending expansions are properly resumed
- Performance benchmarking was needed to compare blocking vs async

This complexity was never fully addressed.

## Why This Approach Failed

The two-phase expansion approach with callback continuation failed because:

1. **Architectural complexity:** The state management required to track pending callbacks, continuation states, and request IDs was too complex to implement correctly in a single attempt.

2. **Callback routing mismatch:** The initial design mixed receiver-based and channel-based routing, creating a fundamental incompatibility that wasn't discovered until late in implementation.

3. **Incomplete resumption logic:** The core logic for resuming expansions when callbacks arrive was never fully implemented, leaving the system in a non-functional state.

4. **Scheduler integration complexity:** Integrating job pausing/resumption into the scheduler added significant complexity that wasn't fully worked out.

5. **Test suite burden:** The test suite required extensive updates to support async behavior, which was never completed.

6. **No incremental path:** The implementation attempted a wholesale replacement rather than an incremental migration, making it difficult to validate each component independently.

## Alternative Approach: Strip Synchronous Planner

Given the complexity of making the callback architecture fully async while maintaining both synchronous and synchronous planners, a simpler approach is to:

1. **Migrate to async-only planner:** Remove the synchronous planner entirely and use only the async background planner
2. **Document the migration:** Create a concrete migration plan in the notes directory
3. **Leverage existing async infrastructure:** The async planner already exists and works for many cases
4. **Simplify callback handling:** Without the need to support both planners, the callback architecture can be simplified
5. **Focus on working solution:** Rather than implementing a complex two-phase expansion, focus on making the existing async planner work reliably

This approach acknowledges that the two-phase expansion architecture was too complex to implement correctly and that a simpler migration path exists.

## Path Forward: Async-Only Migration Strategy

### Key Insight

The fundamental issue with the two-phase expansion approach was that it tried to make the planner itself non-blocking while still using Rayon's thread pool. This created complex state management problems. A simpler approach is to accept that the planner will block on callbacks, but ensure this blocking happens in a way that doesn't affect the main thread.

### Architecture for Async-Only Planner

The async background planner already exists and works for many cases. The migration strategy is:

1. **Remove synchronous planner entirely:** Delete all synchronous planning code paths
2. **Make async planner the only planner:** All planning requests go through the async scheduler
3. **Accept blocking in worker threads:** Worker threads can block on callbacks - this is acceptable as long as the main thread isn't blocked
4. **Simplify callback routing:** With only one planner, there's no need for complex callback routing between different planner instances
5. **Leverage existing callback infrastructure:** The current callback infrastructure (request/response channels) works correctly for the async planner

### Avoiding the Callback Routing Mismatch

The callback routing mismatch occurred because the implementation tried to mix two incompatible approaches. To avoid this in the future:

1. **Choose one routing approach and stick with it:** Either receiver-based routing OR channel-based routing, not both
2. **If using receiver-based routing:** Each pending expansion stores its own receiver, and callbacks are sent directly to that receiver
3. **If using channel-based routing:** Use a single global channel with request IDs to match callbacks to pending expansions
4. **Don't mix approaches:** The implementation should never have both receivers and a global channel for the same purpose

### Recommended Approach: Channel-Based Routing with Request IDs

For the async-only planner, channel-based routing with request IDs is the recommended approach:

1. **Single global callback channel:** The scheduler sends all callback results to a single channel
2. **Request ID generation:** Each callback request gets a unique request ID
3. **Pending expansion tracking:** Pending expansions store the request IDs they're waiting for
4. **Callback matching:** When a callback arrives, the planner finds all pending expansions waiting for that request ID
5. **Resumption logic:** Each matched pending expansion is resumed with the callback result

This approach is simpler than receiver-based routing because:
- No need to manage multiple channels
- Request IDs are easier to track than channel receivers
- The global channel can be shared across all planner instances
- Callback matching is straightforward with request IDs

### Implementation Steps for Async-Only Migration

1. **Audit current planner usage:** Identify all places where the synchronous planner is called
2. **Replace with async planner:** Replace all synchronous planner calls with async planner calls
3. **Update test suite:** Update tests to use the async planner and process callbacks appropriately
4. **Remove synchronous planner code:** Delete all synchronous planner implementation code
5. **Simplify callback infrastructure:** Remove any callback infrastructure that was added specifically for the two-phase expansion approach
6. **Verify correctness:** Run full test suite to ensure async planner works correctly for all use cases

### Leveraging Existing Infrastructure

The following infrastructure from the failed two-phase expansion attempt can be reused:

1. **Notification channel:** The `PlannerCallback` enum and `planner_callback_tx` channel are useful for callback routing
2. **Request ID generation:** The request ID counter and assignment logic is correct and should be kept
3. **Load spreading:** The per-frame callback budget is valuable for preventing frame hitches
4. **Pending expansion types:** The `PendingExpansion` and `PendingCallback` types are well-designed and should be kept

The following infrastructure should be removed:

1. **Receiver-based routing:** Remove any receivers stored in `PendingCallback` variants
2. **Two-phase expansion logic:** Remove the complex expansion continuation logic
3. **Blocking methods:** Remove any `.block()` methods that were added for backward compatibility
4. **Scheduler job pausing:** Remove any job pausing/resumption logic that was added for the two-phase approach

### Lessons Learned

1. **Don't mix routing approaches:** Choose one callback routing approach and stick with it consistently
2. **Simplify state management:** The more state you need to track (pending callbacks, continuations, request IDs), the harder it is to get right
3. **Incremental validation:** Implement and validate each component independently before integrating
4. **Accept trade-offs:** Making the planner fully non-blocking may not be worth the complexity if blocking in worker threads is acceptable
5. **Leverage existing solutions:** The async planner already works for many cases - focus on making it work reliably rather than implementing a complex new architecture

### Success Criteria

The async-only migration is successful when:

1. All synchronous planner code is removed
2. All planning requests go through the async scheduler
3. The full test suite passes with the async planner
4. Callback processing works correctly without frame hitches
5. The callback architecture is simple and maintainable
6. No complex state management is required for callback routing
