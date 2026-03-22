# GdPlanningAI Rust Migration Analysis
## Comprehensive Technical Specification for Core Simulation Engine Migration

**Analysis Date**: March 12, 2026  
**Primary Goal**: Architectural clarity through separation of simulation logic from Godot integration  
**Deliverable Level**: Detailed technical specification with interface definitions and integration patterns

---

## Executive Summary

GdPlanningAI implements a sophisticated Goal-Oriented Action Planning (GOAP) system with novel object-oriented features. The core planning algorithm operates on simulated state snapshots, making it an excellent candidate for Rust migration. This analysis identifies a clean architectural boundary between the **simulation engine** (pure computational logic) and the **Godot integration layer** (scene tree, navigation, lifecycle management).

**Key Findings**:
- ~70% of planning logic is Godot-independent and suitable for Rust migration
- Clear interface boundary exists at the blackboard/action abstraction level
- Rust migration would improve architectural clarity, testability, and maintainability
- GDExtension provides a mature path for Rust-Godot integration

**User Experience Guarantee**:
- Users continue writing actions and goals **purely in GDScript**
- No Rust knowledge required for addon users
- Rust handles only the planning algorithm core
- Custom logic (preconditions, effects) uses callback mechanism to GDScript

---

## Part 1: Architecture Analysis

### 1.1 System Component Map

```
┌─────────────────────────────────────────────────────────────────┐
│                    GdPlanningAI Architecture                     │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────┐         ┌──────────────────────┐     │
│  │   Godot Integration  │         │   Simulation Engine  │     │
│  │        Layer         │         │   (Rust Candidate)   │     │
│  └──────────────────────┘         └──────────────────────┘     │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ GdPAIAgent                                               │   │
│  │ ├─ Thread management                                     │   │
│  │ ├─ Timer-based planning strategies                       │   │
│  │ ├─ Scene tree queries (world state collection)           │   │
│  │ └─ Plan execution coordination                           │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                      │
│                           ▼                                      │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Plan (Core Planning Algorithm)                           │   │
│  │ ├─ Recursive tree building                               │   │
│  │ ├─ State simulation & copying                            │   │
│  │ ├─ Action cost evaluation                                │   │
│  │ ├─ Precondition checking                                 │   │
│  │ └─ Goal satisfaction verification                        │   │
│  └─────────────────────────────────────────────────────────┘   │
│                           │                                      │
│                           ▼                                      │
│  ┌───────────────────────┐    ┌────────────────────────────┐   │
│  │ GdPAIBlackboard       │    │ Action Framework           │   │
│  │ ├─ Dictionary storage │    │ ├─ Action (base)           │   │
│  │ ├─ Copy mechanism     │    │ ├─ SpatialAction (nav)     │   │
│  │ └─ Object tracking    │    │ ├─ Precondition evaluation │   │
│  └───────────────────────┘    │ └─ Goal definition         │   │
│                                └────────────────────────────┘   │
│                                                                  │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ GdPAIObjectData / GdPAIWorldNode                         │   │
│  │ ├─ Scene tree node attachment                            │   │
│  │ ├─ Action broadcasting                                   │   │
│  │ └─ World state snapshot creation                         │   │
│  └─────────────────────────────────────────────────────────┘   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

### 1.2 Core Planning Algorithm Analysis

**File**: `addons/GdPlanningAI/scripts/refcounteds/plan.gd`

The planning algorithm is the heart of the simulation engine:

```gdscript
# Core planning flow (simplified)
func _build_plan(node, prior_actions, blackboard, world_state, recursion_level):
    # 1. Check recursion depth
    if recursion_level > _max_recursion:
        return false
    
    # 2. Check if goal satisfied
    if _is_goal_satisfied(node["desired_state"]):
        return true
    
    # 3. For each available action:
    for action in _available_actions:
        # Clone states for simulation
        sim_blackboard = blackboard.copy_for_simulation()
        sim_world_state = world_state.copy_for_simulation()
        
        # Calculate cost
        sim_cost = await action.get_action_cost(sim_blackboard, sim_world_state)
        
        # Simulate effect
        action.simulate_effect(sim_blackboard, sim_world_state)
        
        # Backpropagate with prior actions
        for prior_act in prior_actions:
            prior_act.reverse_simulate_effect(sim_blackboard, sim_world_state)
        
        # Evaluate if action makes progress toward goal
        should_use_action = await _evaluate_goals(...)
        
        # Recurse if promising
        if should_use_action:
            if await _build_plan(...):
                node.children.append(next_node)
    
    return has_solution
```

**Computational Characteristics**:
- **Stateless**: Each recursion level works on cloned state snapshots
- **Pure Functions**: Simulation effects don't modify original state
- **Tree Building**: Constructs plan tree through recursive exploration
- **Cost-Based Pruning**: Uses action costs to guide search

**Migration Viability**: ✅ **EXCELLENT** - Entire algorithm is pure logic with no Godot dependencies

### 1.3 State Management Analysis

**File**: `addons/GdPlanningAI/scripts/refcounteds/gdpai_blackboard.gd`

```gdscript
class_name GdPAIBlackboard
extends RefCounted

const GDPAI_OBJECTS = "GDPAI_OBJECTS"
var is_a_copy: bool
var _blackboard: Dictionary = { GDPAI_OBJECTS: [] }

func copy_for_simulation() -> GdPAIBlackboard:
    var duplicate: GdPAIBlackboard = GdPAIBlackboard.new()
    # Copy primitive properties
    for key in _blackboard.keys():
        if key == GDPAI_OBJECTS:
            continue
        duplicate._blackboard[key] = _blackboard[key]
    
    # Deep copy object data
    var duped_objects: Array[GdPAIObjectData] = []
    for aod in _blackboard[GDPAI_OBJECTS]:
        if aod != null and is_instance_valid(aod):
            duped_objects.append(aod.copy_for_simulation())
    
    duplicate._blackboard[GDPAI_OBJECTS] = duped_objects
    return duplicate
```

**Data Structure**:
- Dictionary-based key-value store
- Special handling for `GDPAI_OBJECTS` array
- Deep copy mechanism for simulation isolation

**Migration Viability**: ✅ **GOOD** - Core dictionary operations are portable, but object copying needs bridge

### 1.4 Action Framework Analysis

**Base Action** (`action.gd`):
```gdscript
class_name Action
extends RefCounted

enum Status { FAILURE, RUNNING, SUCCESS }
var uid: String

# Simulation methods (pure logic)
func get_action_cost(agent_blackboard, world_state) -> float
func get_preconditions() -> Array[Precondition]
func simulate_effect(agent_blackboard, world_state)
func reverse_simulate_effect(agent_blackboard, world_state)

# Execution methods (Godot-dependent)
func pre_perform_action(agent) -> Status
func perform_action(agent, delta) -> Status
func post_perform_action(agent) -> Status
```

**SpatialAction** (`spatial_action.gd`):
```gdscript
class_name SpatialAction
extends Action

var object_location: GdPAILocationData
var interactable_attribs: GdPAIInteractable

# Cost calculation uses Euclidean distance (pure math)
func get_action_cost(agent_blackboard, world_state) -> float:
    var dist = (agent_location.position - sim_location.position).length()
    return dist

# Simulation effect (pure math)
func simulate_effect(agent_blackboard, world_state):
    agent_location.position = sim_location.position

# Execution uses NavigationAgent (Godot-dependent)
func perform_action(agent, delta):
    nav_agent.target_position = object_location.position
    # ... navigation logic ...
```

**Key Insight**: Actions have a **dual nature**:
- **Simulation phase**: Pure mathematical/logical operations
- **Execution phase**: Godot scene tree and navigation integration

**Migration Viability**: 
- Simulation methods: ✅ **EXCELLENT** 
- Execution methods: ❌ **GODOT-SPECIFIC** (keep in GDScript)

### 1.5 Precondition System Analysis

**File**: `addons/GdPlanningAI/scripts/refcounteds/precondition.gd`

```gdscript
class_name Precondition
extends RefCounted

enum Target { AGENT, WORLD_STATE }
enum Operation { HAS_PROPERTY, EQUAL, NOT_EQUAL, GREATER_THAN, ... }

var is_satisfied: bool = false
var eval_func: Callable

# Static factory methods for common preconditions
static func agent_property_greater_than(prop: String, value: Variant) -> Precondition
static func world_state_has_object_data_of_group(group: String) -> Precondition

# Evaluation
func evaluate(blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard) -> bool:
    if is_satisfied:
        return true
    is_satisfied = eval_func.call(blackboard, world_state)
    return is_satisfied
```

**Migration Viability**: ✅ **GOOD** - Core evaluation logic is portable, but:
- Callable system needs Rust equivalent (function pointers/closures)
- `is_instance_valid()` checks need bridge to Godot's object validity system

---

## Part 2: Godot Dependency Analysis

### 2.1 Dependency Classification

| Component | Godot Dependencies | Migration Complexity |
|-----------|-------------------|---------------------|
| **Plan.gd** | None (pure logic) | 🟢 Low |
| **GdPAIBlackboard.gd** | `is_instance_valid()` | 🟡 Medium |
| **Action.gd** (simulation) | None | 🟢 Low |
| **Action.gd** (execution) | Heavy (Node, scene tree) | 🔴 High |
| **Precondition.gd** | `is_instance_valid()`, Callable | 🟡 Medium |
| **Goal.gd** | None | 🟢 Low |
| **SpatialAction.gd** (simulation) | None | 🟢 Low |
| **SpatialAction.gd** (execution) | NavigationAgent, scene tree | 🔴 High |
| **GdPAIAgent.gd** | Thread, Timer, Node, scene tree | 🔴 High |
| **GdPAIObjectData.gd** | Node, scene tree groups | 🔴 High |
| **GdPAIWorldNode.gd** | Scene tree traversal | 🔴 High |

### 2.2 Interface Boundary Identification

**Natural Separation Point**: Between **planning/simulation** and **execution/world interaction**

```
┌─────────────────────────────────────────────────────────────┐
│                    BOUNDARY INTERFACE                        │
├─────────────────────────────────────────────────────────────┤
│                                                              │
│  GDScript Layer (Godot)          │  Rust Layer (Simulation) │
│  ────────────────────────────────┼──────────────────────────│
│                                   │                          │
│  • Agent lifecycle                │  • Plan tree building    │
│  • World state collection         │  • State simulation      │
│  • Action execution               │  • Cost calculation      │
│  • Navigation integration         │  • Precondition eval     │
│  • Scene tree queries             │  • Goal satisfaction     │
│  • Object validity tracking       │  • Blackboard copying    │
│                                   │                          │
│  ════════════════════════════════╳════════════════════════ │
│              Data Exchange Interface                          │
│  ════════════════════════════════╳════════════════════════ │
│                                   │                          │
│  Input to Rust:                   │  Output from Rust:       │
│  • Serialized blackboard state    │  • Plan action chain     │
│  • Action definitions             │  • Plan cost             │
│  • Goal definitions               │  • Success/failure       │
│  • Validity flags                 │  • Debug tree data       │
│                                   │                          │
└─────────────────────────────────────────────────────────────┘
```

### 2.3 Godot-Specific Patterns Requiring Bridge

**1. Object Validity Tracking**
```gdscript
# GDScript
if is_instance_valid(object):
    # use object
```
**Rust Bridge Solution**: Maintain a validity registry that GDScript updates

**2. Callable System (CRITICAL for User Experience)**
```gdscript
# GDScript - User writes custom precondition
var precondition = Precondition.new()
precondition.eval_func = func(blackboard, world_state):
    return blackboard.get_property("hunger") > 50
```
**Rust Bridge Solution**: Callback mechanism - Rust calls back to GDScript evaluator

**How it works**:
1. Rust planning engine encounters `CustomPrecondition` with unique ID
2. Rust calls GDScript bridge: `evaluate_custom_precondition(id, blackboard, world_state)`
3. GDScript bridge invokes the user's callable
4. Result returns to Rust for planning decisions

**This ensures users never write Rust code!**

**3. Scene Tree Groups**
```gdscript
# GDScript
for obj in get_tree().get_nodes_in_group("GdPAIObjectData"):
    objects.append(obj)
```
**Rust Bridge Solution**: GDScript collects and serializes objects before passing to Rust

---

## Part 3: Interface Specification

### 3.1 Core Data Structures (Rust)

```rust
// Blackboard state representation
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BlackboardState {
    pub properties: HashMap<String, PropertyValue>,
    pub objects: Vec<ObjectDataSnapshot>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PropertyValue {
    Float(f64),
    Int(i64),
    Bool(bool),
    String(String),
    Vector2(Vector2),
    Vector3(Vector3),
    Null,
}

// Object data snapshot (decoupled from Godot nodes)
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectDataSnapshot {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, PropertyValue>,
    pub position: Option<Vector3>, // For spatial objects
}

// Action definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub uid: String,
    pub action_type: ActionType,
    pub cost: f64,
    pub preconditions: Vec<PreconditionDef>,
    pub validity_checks: Vec<PreconditionDef>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActionType {
    SelfAction,
    SpatialAction {
        target_object_uid: String,
    },
}

// Precondition definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreconditionDef {
    pub target: PreconditionTarget,
    pub operation: PreconditionOp,
    pub property_name: String,
    pub value: Option<PropertyValue>,
    pub is_satisfied: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreconditionTarget {
    Agent,
    WorldState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreconditionOp {
    // Built-in operations (handled entirely in Rust for speed)
    HasProperty,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    
    // Custom operations (callback to GDScript)
    // Users write these in GDScript - no Rust needed!
    CustomCallback {
        precondition_id: String,  // Unique ID to identify user's callable
    },
}

// Goal definition
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalDefinition {
    pub uid: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionDef>,
}

// Plan result
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanResult {
    pub success: bool,
    pub action_chain: Vec<String>, // UIDs of actions in order
    pub total_cost: f64,
    pub plan_tree: Option<PlanTreeNode>, // For debugging
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PlanTreeNode {
    pub action_uid: String,
    pub cost: f64,
    pub children: Vec<PlanTreeNode>,
}
```

### 3.2 Rust Planning Engine Interface

```rust
pub struct PlanningEngine {
    max_recursion: usize,
}

impl PlanningEngine {
    pub fn new(max_recursion: usize) -> Self {
        Self { max_recursion }
    }
    
    /// Main planning entry point
    pub fn build_plan(
        &mut self,
        agent_blackboard: BlackboardState,
        world_state: BlackboardState,
        available_actions: Vec<ActionDefinition>,
        goals: Vec<GoalDefinition>,
    ) -> PlanResult {
        // Sort goals by reward (descending)
        let sorted_goals = self.sort_goals_by_reward(goals);
        
        // Try each goal until one succeeds
        for goal in sorted_goals {
            let result = self.try_build_plan_for_goal(
                agent_blackboard.clone(),
                world_state.clone(),
                &available_actions,
                &goal,
            );
            
            if result.success {
                return result;
            }
        }
        
        PlanResult {
            success: false,
            action_chain: vec![],
            total_cost: f64::INFINITY,
            plan_tree: None,
        }
    }
    
    fn try_build_plan_for_goal(
        &mut self,
        agent_blackboard: BlackboardState,
        world_state: BlackboardState,
        actions: &[ActionDefinition],
        goal: &GoalDefinition,
    ) -> PlanResult {
        // Implementation of recursive planning algorithm
        // ...
    }
    
    fn evaluate_precondition(
        &self,
        precondition: &PreconditionDef,
        agent_blackboard: &BlackboardState,
        world_state: &BlackboardState,
    ) -> bool {
        let source = match precondition.target {
            PreconditionTarget::Agent => agent_blackboard,
            PreconditionTarget::WorldState => world_state,
        };
        
        match &precondition.operation {
            PreconditionOp::HasProperty => {
                source.properties.contains_key(&precondition.property_name)
            }
            PreconditionOp::GreaterThan => {
                let prop = source.properties.get(&precondition.property_name);
                let val = precondition.value.as_ref();
                // Compare property > value
                // ...
            }
            // ... other operations
            PreconditionOp::CustomCallback { precondition_id } => {
                // Call back to GDScript to evaluate user's custom precondition
                // This is how users write logic in GDScript without touching Rust!
                self.bridge.evaluate_custom_precondition(
                    precondition_id,
                    agent_blackboard,
                    world_state,
                )
            }
        }
    }
    
    fn simulate_action_effect(
        &mut self,
        action: &ActionDefinition,
        agent_blackboard: &mut BlackboardState,
        world_state: &mut BlackboardState,
    ) {
        // Check if this is a built-in action or custom action
        match &action.effect_type {
            EffectType::BuiltIn(effect) => {
                // Built-in effects handled in Rust for speed
                self.apply_builtin_effect(effect, agent_blackboard, world_state);
            }
            EffectType::CustomCallback { action_id } => {
                // Custom effects: callback to GDScript
                // Users write these in GDScript - no Rust needed!
                self.bridge.simulate_custom_action_effect(
                    action_id,
                    agent_blackboard,
                    world_state,
                );
            }
        }
    }
}
```

### 3.3 GDScript Bridge Interface

**Key Principle**: This bridge is internal infrastructure. Users never interact with it directly.

```gdscript
# gdpai_rust_bridge.gd (INTERNAL - users don't touch this)
class_name GdPAIRustBridge
extends RefCounted

var planning_engine: RefCounted # GDExtension Rust object
# Registry of user's custom callbacks
var _custom_preconditions: Dictionary = {}  # id -> Callable
var _custom_action_effects: Dictionary = {}  # id -> Action object

func _init():
    planning_engine = GDExtensionRustPlanningEngine.new()

## Called by Rust when it encounters a custom precondition
## Users' GDScript callables are invoked here
func evaluate_custom_precondition(
    precondition_id: String,
    agent_blackboard: Dictionary,
    world_state: Dictionary
) -> bool:
    if precondition_id in _custom_preconditions:
        var callable: Callable = _custom_preconditions[precondition_id]
        # Convert back to GdPAIBlackboard for user's code
        var agent_bb = _deserialize_blackboard(agent_blackboard)
        var world_bb = _deserialize_blackboard(world_state)
        return callable.call(agent_bb, world_bb)
    return false

## Called by Rust when it needs to simulate a custom action effect
## Users' GDScript action.simulate_effect() methods are invoked here
func simulate_custom_action_effect(
    action_id: String,
    agent_blackboard: Dictionary,
    world_state: Dictionary
) -> void:
    if action_id in _custom_action_effects:
        var action: Action = _custom_action_effects[action_id]
        var agent_bb = _deserialize_blackboard(agent_blackboard)
        var world_bb = _deserialize_blackboard(world_state)
        action.simulate_effect(agent_bb, world_bb)

## Register user's custom precondition (called during action serialization)
func register_custom_precondition(callable: Callable) -> String:
    var id = _generate_unique_id()
    _custom_preconditions[id] = callable
    return id

## Register user's action for effect callbacks
func register_action_for_callbacks(action: Action) -> String:
    var id = action.uid
    _custom_action_effects[id] = action
    return id

## Convert GDScript blackboard to Rust-compatible format
func serialize_blackboard(blackboard: GdPAIBlackboard) -> Dictionary:
    var serialized = {
        "properties": {},
        "objects": []
    }
    
    # Copy properties
    for key in blackboard.get_dict():
        if key == GdPAIBlackboard.GDPAI_OBJECTS:
            continue
        serialized.properties[key] = _serialize_property(blackboard.get_property(key))
    
    # Copy objects
    for obj in blackboard.get_property(GdPAIBlackboard.GDPAI_OBJECTS):
        serialized.objects.append(_serialize_object_data(obj))
    
    return serialized

## Convert action to Rust-compatible format
## Handles both built-in and custom preconditions/effects
func serialize_action(action: Action) -> Dictionary:
    var serialized = {
        "uid": action.uid,
        "action_type": _get_action_type(action),
        "cost": 0.0, # Will be calculated during planning
        "preconditions": _serialize_preconditions(action.get_preconditions()),
        "validity_checks": _serialize_preconditions(action.get_validity_checks()),
        "has_custom_effect": true,  # All user actions have custom effects
    }
    
    # Register action for callback during planning
    register_action_for_callbacks(action)
    
    return serialized

## Serialize preconditions, handling custom callables
func _serialize_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
    var serialized = []
    for precondition in preconditions:
        var dict = {
            "target": _get_precondition_target(precondition),
            "operation": _get_precondition_op(precondition),
            "property_name": "",
            "value": null,
            "is_satisfied": false,
        }
        
        # Check if this is a built-in or custom precondition
        if _is_builtin_precondition(precondition):
            # Built-in: extract property and operation
            dict.property_name = _extract_property_name(precondition)
            dict.value = _extract_value(precondition)
        else:
            # Custom: register callable and create callback reference
            var id = register_custom_precondition(precondition.eval_func)
            dict.operation = "custom_callback"
            dict.precondition_id = id
        
        
        serialized.append(dict)
    return serialized

## Main planning entry point
func build_plan(
    agent: GdPAIAgent,
    actions: Array[Action],
    goals: Array[Goal]
) -> Dictionary:
    
    # Serialize inputs
    var agent_state = serialize_blackboard(agent.blackboard)
    var world_state = serialize_blackboard(agent.world_node.get_world_state())
    var serialized_actions = []
    for action in actions:
        serialized_actions.append(serialize_action(action))
    var serialized_goals = []
    for goal in goals:
        serialized_goals.append(serialize_goal(goal, agent))
    
    # Call Rust planning engine
    var result = planning_engine.build_plan(
        agent_state,
        world_state,
        serialized_actions,
        serialized_goals
    )
    
    return result

## Convert plan result back to GDScript
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Array[Action]:
    var action_chain = []
    for action_uid in result.action_chain:
        for action in actions:
            if action.uid == action_uid:
                action_chain.append(action)
                break
    return action_chain
```

### 3.4 User Experience: Writing Custom Actions

**Users write actions EXACTLY as they do now - no changes to their workflow!**

```gdscript
# User's custom action (written in GDScript, same as before)
class_name MyEatFoodAction
extends SpatialAction

var food_item: FoodObject

# User implements these methods in GDScript - Rust calls them via callbacks
func simulate_effect(agent_blackboard: GdPAIBlackboard, world_state: GdPAIBlackboard):
    super(agent_blackboard, world_state)
    var hunger = agent_blackboard.get_property("hunger")
    hunger += food_item.hunger_value  # User's custom logic
    agent_blackboard.set_property("hunger", hunger)

func get_preconditions() -> Array[Precondition]:
    # User creates preconditions in GDScript
    return [
        Precondition.agent_has_property("hunger"),
        Precondition.new(func(bb, ws): return bb.get_property("hunger") < 100)
    ]

func perform_action(agent: GdPAIAgent, delta: float) -> Action.Status:
    # User's runtime logic - stays in GDScript
    # ...
```

**What changes for users**: NOTHING - they continue writing GDScript!
**What changes internally**: The planning algorithm now runs in Rust for speed

### 3.5 Integration Flow

```
┌──────────────────────────────────────────────────────────────┐
│                    Planning Request Flow                      │
│         (Users only see the GDScript layer)                   │
└──────────────────────────────────────────────────────────────┘

1. GDScript: GdPAIAgent._query_world_state_and_plan()
   │
   ├─> Collect world state from scene tree
   ├─> Gather valid actions from objects (user's GDScript actions)
   ├─> Compute goal rewards (user's GDScript goals)
   │
   └─> Call GdPAIRustBridge.build_plan()

2. GDScript: GdPAIRustBridge.build_plan() [INTERNAL - users don't touch]
   │
   ├─> Serialize agent blackboard
   ├─> Serialize world state
   ├─> Serialize actions (register user's GDScript callables for callbacks)
   ├─> Serialize goals
   │
   └─> Call Rust planning_engine.build_plan()

3. Rust: PlanningEngine.build_plan() [INTERNAL - pure algorithm]
   │
   ├─> Sort goals by reward
   ├─> For each goal:
   │   ├─> Clone states for simulation
   │   ├─> Build plan tree recursively
   │   │   ├─> **Check action validity** (NEW: validity_checks field)
   │   │   │   ├─> Built-in: evaluate in Rust (fast)
   │   │   │   └─> Custom: callback to GDScript (user's callable)
   │   │   │       └─> Note: Custom validity checks require Godot echo-back
   │   │   ├─> Evaluate preconditions
   │   │   │   ├─> Built-in: evaluate in Rust (fast)
   │   │   │   └─> Custom: callback to GDScript (user's callable)
   │   │   ├─> Calculate action costs
   │   │   ├─> Simulate action effects
   │   │   │   └─> Callback to user's action.simulate_effect() in GDScript
   │   │   └─> Check goal satisfaction
   │   └─> Return first successful plan
   │
   └─> Return PlanResult

4. GDScript: GdPAIRustBridge.deserialize_plan_result() [INTERNAL]
   │
   ├─> Map action UIDs back to Action objects (user's GDScript actions)
   └─> Return action chain to agent

5. GDScript: GdPAIAgent executes plan (user's action code runs)
   │
   ├─> Call action.pre_perform_action() (GDScript)
   ├─> Call action.perform_action() each frame (GDScript)
   └─> Call action.post_perform_action() (GDScript)
```

**Key Point**: Users' GDScript code is called at every step - they never write Rust!

---

## Part 4: Integration Architecture

### 4.1 GDExtension Setup

**Project Structure**:
```
gdplanningai/
├── addons/
│   └── GdPlanningAI/
│       ├── scripts/              # GDScript (existing + new bridge)
│       │   ├── nodes/            # Agent, WorldNode (users don't modify)
│       │   ├── refcounteds/      # Action, Goal, Precondition (users extend these!)
│       │   ├── resources/        # Config resources
│       │   └── gdpai_rust_bridge.gd  # NEW: Internal bridge (users don't touch)
│       ├── rust/                 # New Rust module (internal, users don't touch)
│       │   ├── Cargo.toml
│       │   ├── src/
│       │   │   ├── lib.rs
│       │   │   ├── planning_engine.rs
│       │   │   ├── blackboard.rs
│       │   │   ├── action.rs
│       │   │   └── precondition.rs
│       │   └── build.rs
│       └── bin/
│           └── libgdplanningai_rust.so  # Compiled library
├── .gdextension               # GDExtension config
├── script_templates/          # User templates (unchanged!)
└── project.godot
```

**User-Facing Files** (users interact with these):
- `scripts/refcounteds/action.gd` - Base class users extend
- `scripts/refcounteds/goal.gd` - Base class users extend
- `scripts/refcounteds/precondition.gd` - Helper class users use
- `script_templates/` - Templates for creating new actions/goals

**Internal Files** (users never touch):
- `scripts/gdpai_rust_bridge.gd` - Bridge to Rust engine
- `rust/` - Rust planning engine implementation

**Cargo.toml**:
```toml
[package]
name = "gdplanningai-rust"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
godot = { git = "https://github.com/godot-rust/gdext", branch = "master" }
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

[build-dependencies]
godot-bindings = { git = "https://github.com/godot-rust/gdext", branch = "master" }
```

**.gdextension**:
```ini
[configuration]
entry_symbol = "gdplanningai_rust_init"
compatibility_minimum = "4.2"

[libraries]
linux.debug.x86_64 = "res://addons/GdPlanningAI/bin/libgdplanningai_rust.so"
linux.release.x86_64 = "res://addons/GdPlanningAI/bin/libgdplanningai_rust.so"
windows.debug.x86_64 = "res://addons/GdPlanningAI/bin/gdplanningai_rust.dll"
windows.release.x86_64 = "res://addons/GdPlanningAI/bin/gdplanningai_rust.dll"
macos.debug = "res://addons/GdPlanningAI/bin/libgdplanningai_rust.dylib"
macos.release = "res://addons/GdPlanningAI/bin/libgdplanningai_rust.dylib"
```

### 4.2 Rust GDExtension Implementation

```rust
// src/lib.rs
use godot::prelude::*;

mod planning_engine;
mod blackboard;
mod action;
mod precondition;

use planning_engine::PlanningEngine;

struct GdPlanningAIExt;

#[gdextension]
unsafe impl ExtensionLibrary for GdPlanningAIExt {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                // Auto-load the Rust planning engine
                register_engine_classes();
            }
            _ => {}
        }
    }
}

fn register_engine_classes() {
    // Register RustPlanningEngine as a Godot class
}

// src/planning_engine.rs
use godot::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct RustPlanningEngine {
    max_recursion: usize,
    #[base]
    base: Base<RefCounted>,
}

#[godot_api]
impl RustPlanningEngine {
    #[func]
    fn build_plan(
        &mut self,
        agent_state: Dictionary,
        world_state: Dictionary,
        actions: Array<Dictionary>,
        goals: Array<Dictionary>,
    ) -> Dictionary {
        // Deserialize inputs
        let agent_board = blackboard::from_godot_dict(agent_state);
        let world_board = blackboard::from_godot_dict(world_state);
        let action_defs = action::from_godot_array(actions);
        let goal_defs = action::goals_from_godot_array(goals);
        
        // Run planning algorithm
        let result = self.plan(agent_board, world_board, action_defs, goal_defs);
        
        // Serialize result
        result.to_godot_dict()
    }
    
    #[func]
    fn set_max_recursion(&mut self, max: usize) {
        self.max_recursion = max;
    }
}
```

### 4.3 Async Boundary Handling

**Challenge**: GDScript uses `await` for async operations, but Rust planning should be synchronous.

**Solution**: Separate async collection phase from sync planning phase.

```gdscript
# In GdPAIAgent.gd
func _query_world_state_and_plan() -> void:
    # Async phase: Collect data from scene tree
    var worldly_actions = await _compute_worldly_actions()
    var self_actions = await _compute_valid_self_actions()
    
    # Sync phase: Call Rust planner (no await needed)
    var result = _rust_bridge.build_plan_sync(
        self, 
        self_actions + worldly_actions
    )
    
    # Apply result
    _current_plan = _rust_bridge.deserialize_plan(result, self_actions + worldly_actions)
```

**Rust Side**: Pure synchronous computation, no async needed.

### 4.4 Memory Management

**Ownership Model**:
- GDScript owns the "real" objects (scene tree nodes)
- Rust owns simulation snapshots (cloned data)
- Bridge serializes/deserializes at boundary

**Lifetime Management**:
```rust
// Rust planning engine works on owned copies
pub fn build_plan(
    &mut self,
    agent_blackboard: BlackboardState,  // Owned copy
    world_state: BlackboardState,        // Owned copy
    // ...
) -> PlanResult {
    // No references to GDScript objects
    // Safe to use in multithreading
}
```

**Object Validity Bridge**:
```gdscript
# GDScript maintains validity registry
var _valid_objects: Dictionary = {}

func _on_object_created(obj: GdPAIObjectData):
    _valid_objects[obj.uid] = true

func _on_object_destroyed(obj: GdPAIObjectData):
    _valid_objects.erase(obj.uid)

# Pass validity info to Rust
func get_validity_snapshot() -> Dictionary:
    return _valid_objects.duplicate()
```

---

## Part 5: Migration Strategy

### 5.1 Phased Migration Plan

**Phase 1: Foundation (Weeks 1-2)**
- Set up Rust project structure with GDExtension
- Implement core data structures (BlackboardState, ActionDefinition, etc.)
- Create serialization/deserialization utilities
- **Implement callback mechanism for custom preconditions/effects**
- Write unit tests for data structures

**Phase 2: Core Engine (Weeks 3-4)**
- Implement PlanningEngine in Rust
- Port planning algorithm from Plan.gd
- Implement precondition evaluation
  - Built-in operations: pure Rust (fast)
  - **Custom operations: callback to GDScript bridge**
- Implement action effect simulation
  - Built-in effects: pure Rust (if any)
  - **Custom effects: callback to user's GDScript code**
- Implement goal satisfaction checking
- Write comprehensive tests

**Phase 3: Bridge Layer (Weeks 5-6)**
- Implement GdPAIRustBridge in GDScript
- Create serialization methods for all data types
- **Implement callback registry for user's GDScript callables**
- **Handle edge cases (invalid objects, custom preconditions)**
- **Ensure user's existing actions/goals work without modification**
- Integration testing

**Phase 4: Integration (Weeks 7-8)**
- Modify GdPAIAgent to use Rust bridge (transparent to users)
- Maintain fallback to GDScript planner
- **Test with existing user actions/goals (no changes required)**
- Performance testing
- Debug visualization integration

**Phase 5: Optimization (Weeks 9-10)**
- Profile and optimize Rust implementation
- Implement parallel planning (Rayon)
- Memory optimization
- Final testing and documentation

### 5.2 Risk Mitigation

| Risk | Mitigation |
|------|------------|
| **Behavioral differences** | Maintain GDScript version as fallback; extensive testing |
| **Performance regression** | Benchmark at each phase; profile serialization overhead |
| **API compatibility** | Keep GDScript API unchanged; bridge handles conversion |
| **Debugging difficulty** | Implement debug tree serialization; maintain visual debugger |
| **Custom precondition complexity** | Support custom evaluators via bridge callbacks |
| **GDExtension compatibility** | Test on all target platforms; use stable godot-rust version |

### 5.3 Fallback Mechanism

```gdscript
# In GdPAIAgent.gd
@export var use_rust_planning: bool = true

func _query_world_state_and_plan() -> void:
    if use_rust_planning and _rust_bridge.is_available():
        var result = await _rust_plan()
        if result.success:
            _apply_plan(result)
            return
    
    # Fallback to GDScript planner
    var result = await _gdscript_plan()
    _apply_plan(result)
```

### 5.4 Testing Strategy

**Unit Tests (Rust)**:
- Blackboard copying and property access
- Precondition evaluation for all operation types
- Action cost calculation
- Plan tree building
- Goal satisfaction checking

**Integration Tests (GDScript)**:
- End-to-end planning scenarios
- Multi-agent stress testing
- Edge cases (invalid objects, empty action sets)
- Performance benchmarks

**Compatibility Tests**:
- Compare GDScript vs Rust planner results
- Verify identical behavior across scenarios
- Visual debugger compatibility

---

## Part 6: Performance Considerations

### 6.1 Expected Improvements

**Computational Performance**:
- Rust's compiled performance vs GDScript interpretation
- No garbage collection pauses during planning
- Better optimization opportunities (SIMD, parallel planning)

**Memory Efficiency**:
- Rust's ownership model eliminates copy overhead where possible
- Explicit memory management vs garbage collection
- Potential for object pooling in Rust

**Architectural Benefits**:
- Clear separation of concerns
- Better testability (pure functions)
- Easier profiling and optimization
- Potential for headless simulation (testing, debugging)

### 6.2 Potential Overhead

**Serialization Cost**:
- Converting GDScript dictionaries to Rust structs
- Object data snapshotting
- Plan result deserialization

**Mitigation**:
- Cache serialized action definitions
- Incremental world state updates
- Optimize hot paths with profiling

### 6.3 Benchmarking Framework

```gdscript
# benchmark_planning.gd
extends Node

func benchmark_planning_performance():
    var scenarios = [
        {"agents": 1, "objects": 10, "goals": 2},
        {"agents": 10, "objects": 50, "goals": 3},
        {"agents": 50, "objects": 200, "goals": 4},
    ]
    
    for scenario in scenarios:
        var gdscript_time = _measure_gdscript_planning(scenario)
        var rust_time = _measure_rust_planning(scenario)
        
        print("Scenario: %s" % scenario)
        print("  GDScript: %.2f ms" % gdscript_time)
        print("  Rust: %.2f ms" % rust_time)
        print("  Speedup: %.2fx" % (gdscript_time / rust_time))
```

---

## Part 7: Recommendations

### 7.1 Immediate Actions

1. **Prototype Core Engine**: Implement minimal Rust planning engine to validate architecture
2. **Benchmark Serialization**: Measure overhead of data conversion
3. **Test GDExtension Integration**: Ensure godot-rust works with target Godot version
4. **Document Custom Preconditions**: Catalog all custom precondition patterns in use

### 7.2 Architecture Improvements

**Separation of Concerns**:
- Keep simulation logic in Rust (pure, testable, fast)
- Keep Godot integration in GDScript (scene tree, navigation)
- Keep user extensibility in GDScript (actions, goals, preconditions)
- Clear interface at blackboard/action abstraction level

**User Experience (CRITICAL)**:
- **Users write actions/goals in GDScript exactly as before**
- **No Rust knowledge required for addon users**
- **Callback mechanism ensures user code runs during planning**
- **Existing actions/goals work without modification**

**Extensibility**:
- Support custom precondition evaluators via GDScript callbacks
- All user actions use callback mechanism for effects
- Plugin architecture for domain-specific optimizations
- Future: other languages (C#, VisualScript) can use same callback interface

### 7.3 Long-term Vision

**Potential Future Enhancements**:
- Parallel planning for multiple agents (Rayon)
- Incremental planning (reuse previous plan fragments)
- Machine learning integration for cost prediction
- Headless simulation server for debugging/testing

**Multi-Language Support**:
- Same callback mechanism works for C#, VisualScript, etc.
- Any language that can implement the Action/Goal interfaces
- Rust planning engine is language-agnostic

**Community Benefits**:
- Clearer architecture documentation
- Better code organization for contributors
- **Users don't need to learn Rust**
- Potential for non-Godot ports (Unity, Unreal) with same callback pattern

---

## Appendix A: File Reference

**Core Simulation Engine (Rust Candidates)**:
- `addons/GdPlanningAI/scripts/refcounteds/plan.gd` - Planning algorithm → Rust
- `addons/GdPlanningAI/scripts/refcounteds/gdpai_blackboard.gd` - State management → Rust (with callback bridge)
- `addons/GdPlanningAI/scripts/refcounteds/action.gd` - Base class → **KEPT IN GDSCRIPT** (users extend this!)
- `addons/GdPlanningAI/scripts/refcounteds/precondition.gd` - Helper class → **KEPT IN GDSCRIPT** (users use this!)
- `addons/GdPlanningAI/scripts/refcounteds/goal.gd` - Base class → **KEPT IN GDSCRIPT** (users extend this!)

**Godot Integration Layer (Keep in GDScript)**:
- `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd` - Agent lifecycle
- `addons/GdPlanningAI/scripts/nodes/gdpai_world_node.gd` - World state collection
- `addons/GdPlanningAI/scripts/nodes/gdpai_object_data.gd` - Object integration
- `addons/GdPlanningAI/scripts/refcounteds/spatial_action.gd` - Navigation integration
- `addons/GdPlanningAI/scripts/resources/*.gd` - Configuration resources

**User Extension Points (GDScript - users write code here)**:
- `addons/GdPlanningAI/scripts/refcounteds/action.gd` - Base class for custom actions
- `addons/GdPlanningAI/scripts/refcounteds/goal.gd` - Base class for custom goals
- `addons/GdPlanningAI/scripts/refcounteds/precondition.gd` - Precondition helpers
- `script_templates/` - Templates for new actions/goals

**New Internal Files (users don't touch)**:
- `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd` - Bridge to Rust engine
- `addons/GdPlanningAI/rust/` - Rust planning engine

**Example Implementations**:
- `addons/GdPlanningAI/examples/hunger/` - Hunger behavior system
- `addons/GdPlanningAI/examples/wander/` - Wandering behavior
- `addons/GdPlanningAI/examples/fruit_tree/` - Object interaction

---

## Appendix B: Glossary

- **GOAP**: Goal-Oriented Action Planning - AI planning system where agents form action chains to achieve goals
- **GDExtension**: Godot 4.x's native extension system for integrating compiled languages
- **Blackboard**: Key-value store for agent/world state
- **Simulation**: Planning phase where actions are tested on cloned state snapshots
- **Execution**: Runtime phase where planned actions are performed in the actual game world
- **Precondition**: Condition that must be satisfied for an action to be valid
- **Validity Check**: Hard constraint checked before action is considered for planning
- **SpatialAction**: Action tied to physical object proximity and navigation

---

**Document Version**: 1.0  
**Last Updated**: March 12, 2026  
**Author**: Cascade AI Analysis System
