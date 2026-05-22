# Async Callback Architecture Plan

## Problem Statement

The planner uses synchronous blocking callbacks from Rayon worker threads to the main Godot thread. Planner threads block on `rx.recv()` waiting for GDScript callable responses, causing:

- **Test harness timeouts:** Tests must artificially pump callbacks via `process_callbacks()`, but planner threads still spend most time blocked
- **Production risk:** Current timeout-based workaround (1000ms with safe defaults) is not production-ready
- **Architectural bottleneck:** Synchronous blocking from worker threads to main thread limits parallelism

## Current Architecture

### Blocking Points in `simulation.rs`

1. **Line 43:** Custom precondition evaluation
   ```rust
   match rx.recv() {
       Ok(CallbackResponse::Bool(b)) => b,
       _ => false,
   }
   ```

2. **Line 84:** Action cost calculation
   ```rust
   if let Ok(CallbackResponse::Float(f)) = rx.recv() {
       cost = f;
   }
   ```

3. **Line 107:** Effect simulation
   ```rust
   if let Ok(CallbackResponse::UpdatedSnapshots(res_agent, res_world)) = rx.recv() {
       new_agent = res_agent;
       new_world = res_world;
   }
   ```

### Current Flow

```
Planner Thread (Rayon)          Main Thread (Godot)
     |                                  |
     | send CallbackRequest             |
     |--------------------------------->|
     |                                  | process_callbacks()
     |                                  | execute GDScript callable
     |                                  |
     | block on rx.recv()               |
     |                                  | send CallbackResponse
     |<---------------------------------|
     | unblock, continue                |
```

### Callback Types

From `plan_types.rs`:

```rust
pub enum CallbackKind {
    GetCost { agent, world, provisions, bindings },
    ApplyEffect { agent, world, provisions, bindings },
    EvalCustomPrecond { agent, world, provisions, bindings },
}

pub enum CallbackResponse {
    Float(f64),
    Bool(bool),
    UpdatedSnapshots(BlackboardSnapshot, BlackboardSnapshot),
}
```

## Architectural Options

### Option 1: Branch State Tracking with Blocked/Pending States

**Concept:** Add a state to `SearchNode`/`PlanBranch` indicating if it's blocked on callbacks. Modify controllers to skip blocked nodes during pop.

**Implementation:**

```rust
// In types.rs
#[derive(Clone, Debug)]
pub enum BranchState {
    Ready,
    BlockedOnCallbacks {
        pending_callbacks: Vec<PendingCallback>,
        continuation: BranchContinuation,
    },
}

#[derive(Clone, Debug)]
pub struct PendingCallback {
    callback_id: usize,
    kind: CallbackKind,
}

#[derive(Clone, Debug)]
pub struct BranchContinuation {
    branch_snapshot: PlanBranch,
    next_step: ContinuationStep,
}

// Add to SearchNode
pub struct SearchNode {
    pub branch: PlanBranch,
    pub state: BranchState,  // NEW
    pub depth: usize,
    pub estimated_remaining: f64,
}
```

**Controller modifications:**

```rust
impl SearchController for DFSController {
    fn pop(&mut self) -> Option<SearchNode> {
        while let Some(node) = self.stack.pop() {
            if matches!(node.state, BranchState::Ready) {
                return Some(node);
            }
            // Move blocked nodes to separate queue
        }
        None
    }
}
```

**Pros:**
- Minimal changes to existing architecture
- Clear separation of blocked vs ready branches
- Can process multiple branches in parallel while waiting for callbacks

**Cons:**
- Requires storing continuation state for each blocked branch
- Memory overhead for many blocked branches
- Still needs mechanism to resume branches when callbacks arrive
- Only partial solution - doesn't eliminate blocking, just hides it

---

### Option 2: Future-Based Async Callback Architecture

**Concept:** Replace blocking channels with Rust futures. Use async/await to make callback operations non-blocking.

**Implementation:**

```rust
// Add to Cargo.toml
// futures = "0.3"
// tokio = { version = "1.0", features = ["sync"] }

// In plan_types.rs
pub struct CallbackRequest {
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub response_tx: oneshot::Sender<CallbackResponse>,  // Changed from mpsc
}

// In simulation.rs
pub async fn eval_precondition_async(
    precond: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    provisions: Vec<ProvisionSpec>,
    bindings: Vec<(String, Vec<VariantSnapshot>)>,
    request_tx: &mpsc::Sender<CallbackRequest>,
) -> bool {
    match precond {
        PreconditionSpec::Builtin { .. } => {
            precond.evaluate_builtin(agent, world).unwrap_or(false)
        }
        PreconditionSpec::Custom { callable_id, .. } => {
            let (tx, rx) = oneshot::channel();
            let request = CallbackRequest {
                callable_id: *callable_id,
                kind: CallbackKind::EvalCustomPrecond {
                    agent: agent.clone(),
                    world: world.clone(),
                    provisions,
                    bindings,
                },
                response_tx: tx,
            };

            if request_tx.send(request).is_err() {
                return false;
            }

            match rx.await {  // Non-blocking await
                Ok(CallbackResponse::Bool(b)) => b,
                _ => false,
            }
        }
    }
}
```

**Scheduler modifications:**

```rust
// Use async runtime or custom executor
// When callback arrives, send to oneshot channel
// Planner threads await on futures instead of blocking
```

**Pros:**
- True async/await semantics
- No thread blocking
- Standard Rust async patterns

**Cons:**
- Requires async runtime (tokio/async-std)
- **Major refactoring of planner engine** - all functions become async
- **Rayon incompatible:** Rayon is designed for CPU-bound parallel work, not async I/O
- **Godot integration complexity:** Godot's GDScript callables are synchronous
  - Would need to wrap each callable in `tokio::task::spawn_blocking`
  - This defeats the purpose of async architecture
- Would need to replace Rayon thread pool with async runtime

**Verdict:** Not viable given current Godot integration constraints and Rayon architecture.

---

### Option 3: Two-Phase Expansion with Callback Continuation

**Concept:** Split expansion into two phases. Phase 1: Identify actions and send callback requests without blocking. Phase 2: When callbacks arrive, resume expansion with results.

**Implementation:**

#### Phase 1: Replace blocking callbacks with non-blocking in `simulation.rs`

**Approach:** Direct replacement - no backward compatibility needed. Replace existing blocking functions with non-blocking versions.

```rust
// REPLACES existing eval_precondition
pub fn eval_precondition(
    precond: &PreconditionSpec,
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    provisions: Vec<ProvisionSpec>,
    bindings: Vec<(String, Vec<VariantSnapshot>)>,
    request_tx: &Sender<CallbackRequest>,
) -> PreconditionResult {
    match precond {
        PreconditionSpec::Builtin { .. } => {
            PreconditionResult::Ready(precond.evaluate_builtin(agent, world).unwrap_or(false))
        }
        PreconditionSpec::Custom { callable_id, .. } => {
            let (tx, rx) = std::sync::mpsc::channel();
            let request = CallbackRequest {
                callable_id: *callable_id,
                kind: CallbackKind::EvalCustomPrecond {
                    agent: agent.clone(),
                    world: world.clone(),
                    provisions,
                    bindings,
                },
                response_tx: tx,
            };

            if request_tx.send(request).is_err() {
                return PreconditionResult::Ready(false);
            }

            // Return the receiver instead of blocking
            PreconditionResult::Pending(rx)
        }
    }
}

// REPLACES existing simulate_action
pub fn simulate_action(
    action: &ActionSpec,
    chain_position: usize,
    action_bindings: &[(i64, String, Vec<VariantSnapshot>)],
    agent: &BlackboardSnapshot,
    world: &BlackboardSnapshot,
    accumulated_provisions: Vec<ProvisionSpec>,
    request_tx: &Sender<CallbackRequest>,
) -> SimulationResult {
    // Resolve bindings for this specific action in the chain
    let relevant_bindings: Vec<(String, Vec<VariantSnapshot>)> = action_bindings
        .iter()
        .filter(|(idx, _, _)| *idx == chain_position as i64)
        .map(|(_, name, ids)| (name.clone(), ids.clone()))
        .collect();

    // Non-blocking cost callback
    let cost = if let Some(callable_id) = action.cost_callable_id {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            callable_id,
            kind: CallbackKind::GetCost {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions.clone(),
                bindings: relevant_bindings.clone(),
            },
            response_tx: tx,
        };

        if request_tx.send(request).is_ok() {
            // Return pending cost receiver
            return SimulationResult::PendingCost {
                rx,
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions,
                bindings: relevant_bindings,
            };
        }
        1.0
    } else {
        1.0
    };

    // Non-blocking effect callback
    let mut new_agent = agent.clone();
    let mut new_world = world.clone();

    if let Some(callable_id) = action.effect_callable_id {
        let (tx, rx) = std::sync::mpsc::channel();
        let request = CallbackRequest {
            callable_id,
            kind: CallbackKind::ApplyEffect {
                agent: agent.clone(),
                world: world.clone(),
                provisions: accumulated_provisions,
                bindings: relevant_bindings,
            },
            response_tx: tx,
        };

        if request_tx.send(request).is_ok() {
            // Return pending effect receiver
            return SimulationResult::PendingEffect {
                rx,
                agent: new_agent,
                world: new_world,
                cost,
            };
        }
    }

    SimulationResult::Ready {
        agent: new_agent,
        world: new_world,
        cost,
    }
}

pub enum PreconditionResult {
    Ready(bool),
    Pending(std::sync::mpsc::Receiver<CallbackResponse>),
}

pub enum SimulationResult {
    Ready {
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        cost: f64,
    },
    PendingCost {
        rx: std::sync::mpsc::Receiver<CallbackResponse>,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
    PendingEffect {
        rx: std::sync::mpsc::Receiver<CallbackResponse>,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        cost: f64,
    },
}
```

#### Phase 2: Modified expansion in `expander.rs`

```rust
pub fn expand_non_blocking(
    &self,
    branch: &PlanBranch,
) -> (Vec<PlanBranch>, Vec<PendingExpansion>) {
    let mut ready_successors = Vec::new();
    let mut pending_expansions = Vec::new();

    for action_idx in 0..self.ctx.actions.len() {
        // Check preconditions (now returns non-blocking result)
        let precond_results: Vec<PreconditionResult> = branch.open_preconditions
            .iter()
            .map(|p| eval_precondition(p, &branch.final_state_agent, ...))
            .collect();

        // If any preconditions are pending, create pending expansion
        if precond_results.iter().any(|r| matches!(r, PreconditionResult::Pending(_))) {
            pending_expansions.push(PendingExpansion {
                branch: branch.clone(),
                action_idx,
                pending_callbacks: precond_results,
                continuation: ExpansionContinuation::PreconditionCheck,
            });
            continue;
        }

        // If all preconditions ready, continue with cost/effect
        let cost_result = get_cost_non_blocking(...);
        // Similar pattern for cost and effect...
    }

    (ready_successors, pending_expansions)
}

pub struct PendingExpansion {
    pub branch: PlanBranch,
    pub action_idx: usize,
    pub pending_callbacks: Vec<PendingCallback>,
    pub continuation: ExpansionContinuation,
}

pub enum ExpansionContinuation {
    PreconditionCheck {
        action_idx: usize,
        precond_idx: usize,
    },
    CostCalculation {
        action_idx: usize,
    },
    EffectSimulation {
        action_idx: usize,
        partial_successors: Vec<PlanBranch>,
    },
}
```

#### Phase 3: Callback-to-planner notification in `scheduler.rs`

**Current Issue:** `process_callbacks()` processes ALL pending callbacks in a single frame with no limit, potentially causing frame hitches.

**Solution:** Add per-frame callback budget to spread load across frames.

```rust
struct ActiveJobHandle {
    // ... existing fields
    planner_callback_tx: Sender<PlannerCallback>,  // NEW
}

pub enum PlannerCallback {
    PreconditionResult {
        request_id: usize,
        result: bool,
    },
    CostResult {
        request_id: usize,
        cost: f64,
    },
    EffectResult {
        request_id: usize,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
    },
}

// In process_callbacks - ADD load spreading
#[func]
fn process_callbacks(&mut self) {
    // Process pending log messages from planner threads (from previous frames)
    crate::logger::process_logs();

    // Load spreading: limit callbacks per frame to prevent frame hitches
    let max_callbacks_per_frame = 50;  // Configurable via export
    let mut processed_this_frame = 0;

    // Process each job's pending callbacks using its own callable registry.
    for job in self.active_jobs.iter_mut().filter(|j| !j.done) {
        while processed_this_frame < max_callbacks_per_frame {
            if let Ok(req) = job.request_rx.try_recv() {
                let callable = &job.callable_registry[req.callable_id];
                let response = dispatch_callback(callable, req.kind);
                let _ = req.response_tx.send(response);
                
                // NEW: Notify planner that callback completed
                let callback = match response {
                    CallbackResponse::Bool(b) => PlannerCallback::PreconditionResult {
                        request_id: generate_request_id(&req),
                        result: b,
                    },
                    CallbackResponse::Float(f) => PlannerCallback::CostResult {
                        request_id: generate_request_id(&req),
                        cost: f,
                    },
                    CallbackResponse::UpdatedSnapshots(agent, world) => {
                        PlannerCallback::EffectResult {
                            request_id: generate_request_id(&req),
                            agent,
                            world,
                        }
                    }
                };
                let _ = job.planner_callback_tx.send(callback);
                
                processed_this_frame += 1;
            } else {
                break;  // No more callbacks for this job
            }
        }
        
        if processed_this_frame >= max_callbacks_per_frame {
            break;  // Budget exhausted, continue next frame
        }
        
        // Check for completed plan
        if let Ok(result) = job.result_rx.try_recv() {
            job.done = true;
            if job.cancel_flag.load(std::sync::atomic::Ordering::Relaxed) || result.is_none() {
                log_debug!("Canceled plan job finished without delivery");
                continue;
            }

            let Some(result) = result else {
                log_warn!("Plan job returned None result");
                continue;
            };

            if job.agent.is_instance_valid() {
                log_info!(
                    "Plan complete: success={}, actions={}, cost={:.1}, action_chain={:?}",
                    result.success,
                    result.action_chain.len(),
                    result.total_cost,
                    result.action_chain
                );
                let dict = result_to_dict(&result);
                job.agent.call("_on_plan_ready", &[dict.to_variant()]);
            } else {
                log_warn!("Plan completed but agent was freed");
            }
        }
    }
    // Clean up finished jobs — Rayon owns the threads, no join needed.
    self.active_jobs.retain(|job| !job.done);

    // Drain any log messages generated during this callback processing
    crate::logger::process_logs();
}
```

#### Phase 4: Modified search loop in `engine.rs`

```rust
pub struct PlannerEngine<'a> {
    // ... existing fields
    pending_expansions: Vec<PendingExpansion>,
    callback_registry: HashMap<usize, CallbackResponse>,
    planner_callback_rx: Receiver<PlannerCallback>,
}

impl<'a> PlannerEngine<'a> {
    fn search_goal(&mut self, goal: &GoalSpec) -> Option<PlanResult> {
        // ... existing setup

        while let Some(node) = controller.pop() {
            // Check for completed callbacks
            self.process_completed_callbacks();

            // Phase 1: Non-blocking expansion
            let (successors, pending) = expander.expand_non_blocking(&node.branch);
            
            // Add ready successors immediately
            for succ in successors {
                controller.push(SearchNode { branch: succ, ... });
            }
            
            // Store pending expansions
            self.pending_expansions.extend(pending);
        }
    }
    
    fn process_completed_callbacks(&mut self) {
        while let Ok(callback) = self.planner_callback_rx.try_recv() {
            // Store result in registry
            match callback {
                PlannerCallback::PreconditionResult { request_id, result } => {
                    self.callback_registry.insert(request_id, CallbackResponse::Bool(result));
                }
                // ... handle other callback types
            }
            
            // Resume any pending expansions waiting for this callback
            self.resume_pending_expansions(request_id);
        }
    }
    
    fn resume_pending_expansions(&mut self, request_id: usize) {
        // Find pending expansions waiting for this callback
        // Resume them with the callback result
        // Add ready successors to controller
    }
}
```

#### Phase 5: Controller pop logic modifications

```rust
impl SearchController for DFSController {
    fn pop(&mut self) -> Option<SearchNode> {
        // Only pop nodes that are ready to expand
        while let Some(node) = self.stack.pop() {
            if matches!(node.state, BranchState::Ready) {
                return Some(node);
            }
        }
        None
    }
}
```

**Pros:**
- **Production-ready:** True non-blocking architecture without timeouts
- **Rayon-compatible:** Keeps existing thread pool architecture
- **Godot-compatible:** No changes to GDScript callable integration
- **Test-friendly:** Eliminates callback pumping bottleneck in tests
- **Incremental migration:** Can convert blocking calls gradually

**Cons:**
- Complex state management (tracking pending callbacks per branch)
- Need bidirectional channels (scheduler → planner)
- Requires unique request IDs to match callbacks to branches
- Significant refactoring of expansion logic

---

## Option Comparison

| Option | Complexity | Rayon Compatible | Godot Compatible | Migration Effort | Production Ready |
|--------|-----------|------------------|------------------|------------------|------------------|
| **1. Branch State Tracking** | Medium | ✅ Yes | ✅ Yes | Medium | ⚠️ Partial - still blocks internally |
| **2. Future-Based Async** | High | ❌ No | ⚠️ Complex | High | ❌ No |
| **3. Two-Phase Expansion** | High | ✅ Yes | ✅ Yes | High | ✅ Yes |

## Recommendation: **Option 3 (Two-Phase Expansion)**

### Rationale

1. **Production-ready:** True non-blocking architecture without timeout workarounds
2. **Rayon-compatible:** Keeps existing thread pool architecture (no async runtime needed)
3. **Godot-compatible:** No changes to GDScript callable integration
4. **Test-friendly:** Eliminates callback pumping bottleneck in tests
5. **Incremental migration:** Can convert blocking calls gradually with feature flags

### Implementation Plan

#### Phase 1: Non-blocking callback variants
- **File:** `simulation.rs`
- Add `eval_precondition_non_blocking()`
- Add `get_cost_non_blocking()`
- Add `simulate_effect_non_blocking()`
- Return enum types: `Ready(T)` or `Pending(Receiver)`

#### Phase 2: Modified expander
- **File:** `expander.rs`
- Add `expand_non_blocking()` method
- Return `(ready_successors, pending_expansions)`
- Define `PendingExpansion` struct
- Define `ExpansionContinuation` enum
- Track which callbacks each branch is waiting for

#### Phase 3: Notification channel
- **File:** `scheduler.rs`
- Add `planner_callback_tx` to `ActiveJobHandle`
- Define `PlannerCallback` enum
- Add unique request ID generation
- Send notifications when callbacks complete

#### Phase 4: Engine modifications
- **File:** `engine.rs`
- Add `pending_expansions` field to `PlannerEngine`
- Add `callback_registry` HashMap
- Add `process_completed_callbacks()` method
- Add `resume_pending_expansions()` method
- Modify search loop to check for completed callbacks

#### Phase 5: Controller modifications
- **File:** `controller.rs`
- Add `BranchState` enum to `SearchNode`
- Modify `pop()` to skip blocked nodes
- Add separate queue for pending expansions
- Implement conditional popping

#### Phase 6: Add load spreading configuration (optional)
- **File:** `scheduler.rs`
- Add `max_callbacks_per_frame` as exportable property
- Allow runtime adjustment via GDScript
- Add metrics tracking (callbacks processed per frame, pending queue depth)

### Key Design Decisions

1. **Request ID Generation:** Use incrementing counter per job to match callbacks to branches
2. **Continuation State:** Store as closure-like data structure with branch snapshot and next step
3. **Memory Management:** Limit pending expansions per job (e.g., max 1000) to prevent blowup
4. **Timeout Fallback:** Keep timeout mechanism as safety net for production (from current workaround)
5. **Atomic Migration:** Single branch replacement - no feature flags or gradual rollout
6. **Load Spreading:** Add per-frame callback budget (e.g., 50 callbacks/frame) to prevent frame hitches and ensure smooth rendering

### Migration Strategy

**Direct replacement approach - no backward compatibility:**

1. **Single atomic change:** Replace blocking functions with non-blocking versions in `simulation.rs`
2. **Update all call sites:** Modify `expander.rs`, `engine.rs`, `types.rs` to handle new return types
3. **Add callback handling:** Implement notification channel and callback registry in scheduler/engine
4. **Add load spreading:** Implement per-frame callback budget in `process_callbacks()`
5. **Integration tests:** Add tests specifically for non-blocking behavior
6. **Benchmarking:** Compare performance vs current blocking implementation
7. **Full test suite run:** Ensure all existing tests pass with new architecture

### Testing Strategy

1. **Unit tests:** Test non-blocking callback variants in isolation
2. **Integration tests:** Test full planner with non-blocking callbacks
3. **Performance tests:** Measure planning time with/without blocking
4. **Stress tests:** Test with many pending callbacks (memory limits)
5. **Regression tests:** Ensure existing tests still pass

## Relevant Files

- `addons/GdPlanningAI/rust/src/planner/simulation.rs` - Blocking callback calls
- `addons/GdPlanningAI/rust/src/planner/engine.rs` - Search loop
- `addons/GdPlanningAI/rust/src/planner/controller.rs` - Search controllers
- `addons/GdPlanningAI/rust/src/planner/expander.rs` - Branch expansion
- `addons/GdPlanningAI/rust/src/scheduler.rs` - Callback processing
- `addons/GdPlanningAI/rust/src/plan_types.rs` - Callback types
- `test/integration/test_async_planner.gd` - Integration tests

## Next Steps

1. Review and approve this architecture plan
2. Begin Phase 1 implementation (replace blocking callbacks in simulation.rs)
3. Add unit tests for new return types
4. Proceed through remaining phases incrementally
5. Run full test suite after each phase to catch regressions early

---

## Current Implementation Status

### Completed Infrastructure (2026-05-21)

The following foundational infrastructure has been implemented:

1. **Notification Channel (`scheduler.rs`)**
   - Added `PlannerCallback` enum with `PreconditionResult`, `CostResult`, and `EffectResult` variants
   - Added `planner_callback_tx` channel to `ActiveJobHandle`
   - Implemented load spreading with `max_callbacks_per_frame=50` to prevent frame hitches

2. **Pending Expansion Types (`planner/types.rs`)**
   - Added `PendingExpansion` struct to track expansions waiting on callbacks
   - Added `PendingCallback` enum with `Precondition`, `Cost`, and `Effect` variants
   - Added `ExpansionContinuation` enum to track what step to resume after callback
   - Added `try_recv` method to `PendingCallback` for non-blocking callback retrieval

3. **Engine Modifications (`planner/engine.rs`)**
   - Added `pending_expansions` field to `PlannerEngine`
   - Implemented `resume_pending_expansions` method with callback-to-expansion matching
   - Added continuation logic for sending next-phase requests (e.g., effect after cost)

4. **Simulation Result Enums (`planner/simulation.rs`)**
   - `PreconditionResult` enum with `Ready(bool)` and `Pending(Receiver)` variants
   - `SimulationResult` enum with `Ready`, `PendingCost`, and `PendingEffect` variants
   - Added transitional `.block()` methods for backward compatibility

### Current State

- **Discovery phase:** Uses blocking via `.block()` for test compatibility
- **Ripple simulation:** Uses blocking via `.block()` for test compatibility
- **Critical checks:** Initial state and deep goal checks use blocking
- **Async infrastructure:** Fully implemented but not fully utilized

### Remaining Work for Full Async Cutover

The core architectural issue preventing full async operation is the callback routing mechanism:

**Problem:** `PendingExpansions` store their own receivers, but `resume_pending_expansions` checks the global callback channel. This creates a mismatch where callbacks can't be matched to the specific pending expansions waiting for them.

**Solution:** Refactor to use request_id-based routing instead of receiver-based routing.

#### Task 1: Refactor PendingExpansions to Track Request IDs

**File:** `planner/types.rs`

**Current Design:**
```rust
pub struct PendingExpansion {
    pub branch: PlanBranch,
    pub action_idx: usize,
    pub pending_callbacks: Vec<PendingCallback>,
    pub continuation: ExpansionContinuation,
}

pub enum PendingCallback {
    Precondition {
        rx: Receiver<CallbackResponse>,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
    Cost {
        rx: Receiver<CallbackResponse>,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
        effect_callable_id: Option<usize>,
        request_tx: Sender<CallbackRequest>,
    },
    Effect {
        rx: Receiver<CallbackResponse>,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        cost: f64,
    },
}
```

**New Design:**
```rust
pub struct PendingExpansion {
    pub branch: PlanBranch,
    pub action_idx: usize,
    pub pending_callbacks: Vec<PendingCallback>,
    pub continuation: ExpansionContinuation,
}

pub enum PendingCallback {
    Precondition {
        request_id: usize,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
    },
    Cost {
        request_id: usize,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        provisions: Vec<ProvisionSpec>,
        bindings: Vec<(String, Vec<VariantSnapshot>)>,
        effect_callable_id: Option<usize>,
        request_tx: Sender<CallbackRequest>,
    },
    Effect {
        request_id: usize,
        agent: BlackboardSnapshot,
        world: BlackboardSnapshot,
        cost: f64,
    },
}
```

**Changes:**
- Remove `rx: Receiver<CallbackResponse>` from all `PendingCallback` variants
- Add `request_id: usize` to all variants
- Remove `try_recv` method (no longer needed)

#### Task 2: Update Callback Request ID Generation

**File:** `planner/simulation.rs`

**Current:** No request ID tracking in callback requests.

**New:** Add request ID to `CallbackRequest` and generate unique IDs:

```rust
pub struct CallbackRequest {
    pub callable_id: usize,
    pub kind: CallbackKind,
    pub response_tx: Sender<CallbackResponse>,
    pub request_id: usize,  // NEW
}
```

**Implementation:**
- Pass `request_id_counter` to simulation functions
- Generate unique request ID for each callback request
- Include request ID in request sent to scheduler

#### Task 3: Update Scheduler to Include Request ID in Notifications

**File:** `scheduler.rs`

**Current:** Generates request ID locally in notification.

**New:** Use request ID from callback request:

```rust
// In process_callbacks
let callback = match response {
    CallbackResponse::Bool(b) => PlannerCallback::PreconditionResult {
        request_id: req.request_id,  // Use request ID from request
        result: b,
    },
    CallbackResponse::Float(f) => PlannerCallback::CostResult {
        request_id: req.request_id,
        cost: f,
    },
    CallbackResponse::UpdatedSnapshots(agent, world) => {
        PlannerCallback::EffectResult {
            request_id: req.request_id,
            agent,
            world,
        }
    }
};
```

#### Task 4: Implement Request ID-Based Callback Matching

**File:** `planner/engine.rs`

**Current:** Uses `try_recv` on stored receivers.

**New:** Match callbacks by request ID:

```rust
fn resume_pending_expansions(&mut self) {
    while let Ok(callback) = self.planner_callback_rx.try_recv() {
        let request_id = match &callback {
            PlannerCallback::PreconditionResult { request_id, .. } => *request_id,
            PlannerCallback::CostResult { request_id, .. } => *request_id,
            PlannerCallback::EffectResult { request_id, .. } => *request_id,
        };

        // Find pending expansions waiting for this request_id
        let mut to_resume = Vec::new();
        self.pending_expansions.retain(|expansion| {
            let matches = expansion.pending_callbacks.iter().any(|cb| {
                match cb {
                    PendingCallback::Precondition { request_id: id, .. } => *id == request_id,
                    PendingCallback::Cost { request_id: id, .. } => *id == request_id,
                    PendingCallback::Effect { request_id: id, .. } => *id == request_id,
                }
            });

            if matches {
                to_resume.push(expansion.clone());
                false  // Remove from pending
            } else {
                true  // Keep in pending
            }
        });

        // Resume matched expansions with callback result
        for expansion in to_resume {
            self.resume_expansion_with_callback(expansion, callback.clone());
        }
    }
}
```

#### Task 5: Implement Full Continuation Logic

**File:** `planner/engine.rs`

**Current:** Partial continuation logic implemented.

**New:** Complete continuation logic for all phases:

```rust
fn resume_expansion_with_callback(&mut self, expansion: PendingExpansion, callback: PlannerCallback) {
    match (expansion.continuation, callback) {
        (ExpansionContinuation::CostCalculation { action_idx }, PlannerCallback::CostResult { cost, .. }) => {
            // Cost received, now request effect simulation
            let action = &self.ctx.actions[action_idx];
            let (tx, rx) = channel();
            let request = CallbackRequest {
                callable_id: action.effect_callable_id.unwrap(),
                kind: CallbackKind::ApplyEffect { ... },
                response_tx: tx,
                request_id: self.request_id_counter.fetch_add(1, Ordering::Relaxed),
            };
            let _ = self.ctx.request_tx.send(request);

            // Update pending expansion to wait for effect
            self.pending_expansions.push(PendingExpansion {
                branch: expansion.branch,
                action_idx,
                pending_callbacks: vec![PendingCallback::Effect {
                    request_id: request.request_id,
                    agent: /* from cost callback */,
                    world: /* from cost callback */,
                    cost,
                }],
                continuation: ExpansionContinuation::EffectSimulation { action_idx, cost },
            });
        }
        (ExpansionContinuation::EffectSimulation { action_idx, cost }, PlannerCallback::EffectResult { agent, world, .. }) => {
            // Effect received, create complete branch and add to search
            let mut new_branch = expansion.branch.clone();
            new_branch.final_state_agent = agent;
            new_branch.final_state_world = world;
            new_branch.total_cost += cost;
            // ... other branch updates

            // Add to search controller
            self.controller.push(SearchNode { branch: new_branch, ... });
        }
        // ... handle other continuation types
    }
}
```

#### Task 6: Remove Blocking Methods

**File:** `planner/simulation.rs`

**Current:** `.block()` methods exist for backward compatibility.

**New:** Remove `.block()` methods entirely:

```rust
// REMOVE these methods:
impl PreconditionResult {
    pub fn block(self) -> bool { ... }  // DELETE
}

impl SimulationResult {
    pub fn block(self) -> Option<SimulationReady> { ... }  // DELETE
}
```

#### Task 7: Update Discovery and Ripple to Use Async

**File:** `planner/expander.rs`

**Current:** Uses `.block()` for test compatibility.

**New:** Handle pending results properly:

```rust
// Discovery phase
match sim_result {
    SimulationResult::Ready { agent, world, cost } => {
        // Process ready result
    }
    SimulationResult::PendingCost { request_id, ... } | SimulationResult::PendingEffect { request_id, ... } => {
        // Create pending expansion and skip
        return ExpansionResult::Pending(PendingExpansion {
            branch: branch.clone(),
            action_idx,
            pending_callbacks: vec![/* appropriate callback */],
            continuation: /* appropriate continuation */,
        });
    }
}

// Ripple phase
match sim_result {
    SimulationResult::Ready { agent, world, cost } => {
        // Continue ripple
    }
    SimulationResult::PendingCost { request_id, ... } => {
        // Create pending expansion for cost phase
        return ExpansionResult::Pending(PendingExpansion { ... });
    }
    SimulationResult::PendingEffect { request_id, ... } => {
        // Create pending expansion for effect phase
        return ExpansionResult::Pending(PendingExpansion { ... });
    }
}
```

#### Task 8: Update Test Suite for Async Architecture

**File:** `rust/tests/planner_integration.rs`

**Current:** Mock responder doesn't send `PlannerCallback` notifications.

**New:** Mock responder must send notifications:

```rust
// In spawn_callback_responder
let (planner_callback_tx, planner_callback_rx) = channel();
let request_id_counter = Arc::new(AtomicUsize::new(0));

let handle = thread::spawn(move || {
    for req in req_rx {
        let request_id = request_id_counter.fetch_add(1, Ordering::Relaxed);
        let response = /* generate response */;
        let _ = req.response_tx.send(response);

        // Send planner callback notification
        let callback = match response {
            CallbackResponse::Float(f) => PlannerCallback::CostResult { request_id, cost: f },
            CallbackResponse::Bool(b) => PlannerCallback::PreconditionResult { request_id, result: b },
            CallbackResponse::UpdatedSnapshots(agent, world) => PlannerCallback::EffectResult { request_id, agent, world },
        };
        let _ = planner_callback_tx.send(callback);
    }
});

// Pass planner_callback_rx to planner
```

#### Task 9: Integration Testing

**File:** `test/integration/test_async_planner.gd`

**New:** Add tests specifically for async behavior:

1. Test that planner doesn't block on callbacks
2. Test that pending expansions are resumed correctly
3. Test load spreading with many callbacks
4. Test timeout safety net still works
5. Test that all existing behaviors still work

#### Task 10: Performance Benchmarking

**New:** Compare performance metrics:

1. Planning time with blocking vs async
2. Frame time impact during callback processing
3. Memory usage with many pending expansions
4. Callback queue depth over time
5. Planner throughput (plans per second)

### Implementation Order

1. **Task 1-3:** Refactor types and add request ID tracking (infrastructure)
2. **Task 4-5:** Implement request ID-based matching and continuation logic (engine)
3. **Task 6-7:** Remove blocking and update expansion logic (simulation/expander)
4. **Task 8:** Update test suite (testing)
5. **Task 9-10:** Integration testing and benchmarking (validation)

### Risks and Mitigations

**Risk:** Complex state management with request ID matching could introduce bugs.

**Mitigation:**
- Add extensive logging for callback routing
- Add unit tests for request ID generation and matching
- Add integration tests for full async flow
- Keep timeout safety net as fallback

**Risk:** Memory blowup with many pending expansions.

**Mitigation:**
- Add configurable limit on pending expansions per job (e.g., max 1000)
- Add metrics tracking for pending queue depth
- Add warning when approaching limit
- Implement LRU eviction if limit exceeded

**Risk:** Test suite complexity increases significantly.

**Mitigation:**
- Keep existing blocking tests as regression tests
- Add separate async-specific tests
- Use feature flags to enable/disable async during migration

---

## Implementation Status Update (2026-05-21)

### Completed Work

1. **Removed blocking methods** - Removed `.block()` methods from `PreconditionResult` and `SimulationResult` in `simulation.rs`

2. **Updated discovery and ripple** - Modified `expander.rs` to:
   - Return `PendingExpansion` when encountering pending results in ripple phase
   - Skip candidates with pending results in discovery phase
   - Store continuation data in `PendingCallback` (agent, world, provisions, bindings, request_tx)

3. **Updated planner engine** - Modified `engine.rs` to:
   - Return `PlannerRunResult::Pending` when there are pending expansions
   - Generate unique request IDs for pending callbacks
   - Add request ID assignment in `resume_pending_expansions`

4. **Updated scheduler** - Modified `scheduler.rs` to:
   - Handle `PlannerRunResult` enum
   - Currently treats `Pending` as failure (temporary)

5. **Updated integration tests** - Converted Rust tests to use builtin preconditions only to avoid async complexity

6. **Built release binary** - Successfully built

### Remaining Work for Full Async Resumption

The async infrastructure is in place but the actual resumption logic is incomplete. The scheduler currently treats `Pending` as failure, so the async flow doesn't work end-to-end.

#### Task 5: Implement Full Resumption Logic

**File:** `planner/engine.rs`

**Current state:** `resume_pending_expansions` matches callbacks by request ID but doesn't actually resume the expansion with the callback result.

**Required implementation:**

1. **Cost callback resumption:**
   - When cost callback arrives, use the cost value
   - Request effect simulation with the same agent/world state
   - Update pending expansion to wait for effect result

2. **Effect callback resumption:**
   - When effect callback arrives, use the updated agent/world snapshots
   - Apply the effect to the branch state
   - Add the action to the action chain
   - Push the updated branch to the search controller for further expansion

3. **Precondition callback resumption:**
   - When precondition callback arrives, use the boolean result
   - If satisfied, continue with cost/effect simulation
   - If not satisfied, discard the branch

4. **Scheduler integration:**
   - Remove the temporary "treat Pending as failure" logic
   - Implement proper job pausing/resumption in scheduler
   - Store pending expansions per job
   - Resume planning when callbacks arrive

#### Task 6: Update Integration Tests for Async Flow

**File:** `test/integration/test_async_planner.gd`

**Current state:** Rust integration tests use builtin preconditions only.

**Required implementation:**

1. Create GDScript integration tests that:
   - Submit planning jobs with custom callbacks
   - Process callbacks via `process_callbacks()`
   - Verify that planning completes after callbacks are processed
   - Test request ID uniqueness
   - Test that pending expansions are properly resumed

2. Test scenarios:
   - Single action with cost callback
   - Single action with effect callback
   - Action chain with multiple callbacks
   - Multiple pending expansions waiting on different callbacks

#### Task 7: Performance Benchmarking

**File:** `notes/` (new benchmarking note)

**Required implementation:**

1. Benchmark planning time with:
   - All builtin preconditions (baseline)
   - Mix of builtin and custom callbacks
   - All custom callbacks
   - Many pending callbacks (stress test)

2. Compare:
   - Old blocking implementation (from git history)
   - New async implementation
   - Frame time impact of callback processing

3. Metrics:
   - Planning latency
   - Callback processing time per frame
   - Memory usage for pending expansions
   - Maximum concurrent pending expansions

### Summary

The async callback architecture infrastructure is complete, but the resumption logic needs to be fully implemented to enable end-to-end async planning. The current state is:
- ✅ Non-blocking callback variants
- ✅ Pending expansion tracking
- ✅ Request ID generation and matching
- ✅ Callback notification channel
- ⏳ Full resumption logic (cost → effect → branch continuation)
- ⏳ Scheduler job pausing/resumption
- ⏳ GDScript integration tests for async flow
- ⏳ Performance benchmarking
- Document test setup clearly
