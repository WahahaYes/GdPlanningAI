# GdPlanningAI Rust Migration Implementation Guide
## Concrete Action Plan for Future Agents

**Purpose**: This document provides a detailed, step-by-step implementation guide for migrating the GdPlanningAI core simulation engine to Rust while maintaining full GDScript compatibility for users.

**Target Audience**: Future AI agents or developers implementing the Rust migration

**Prerequisites**:
- Familiarity with Godot 4.x GDExtension system
- Rust development environment setup
- Understanding of GOAP (Goal-Oriented Action Planning)
- Read `RUST_MIGRATION_ANALYSIS.md` for architectural context

---

## Critical Success Criteria

✅ **Users continue writing actions/goals purely in GDScript**  
✅ **No changes required to existing user code**  
✅ **Behavioral parity with current GDScript implementation**  
✅ **Performance improvement (target: 2x speedup)**  
✅ **Maintain debugging and visualization capabilities**

---

## Phase 1: Foundation (Weeks 1-2)

### Step 1.1: Project Setup

**Objective**: Set up Rust project structure with GDExtension integration

**Actions**:
1. Create Rust project directory structure:
```
addons/GdPlanningAI/
├── rust/
│   ├── Cargo.toml
│   ├── Cargo.lock
│   ├── src/
│   │   ├── lib.rs
│   │   ├── planning_engine.rs
│   │   ├── blackboard.rs
│   │   ├── action.rs
│   │   ├── precondition.rs
│   │   ├── goal.rs
│   │   └── bridge.rs
│   └── build.rs
├── bin/
│   ├── linux/   # .so files
│   ├── windows/ # .dll files
│   └── macos/   # .dylib files
└── .gdextension
```

2. Create `Cargo.toml`:
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
rayon = "1.8"  # For parallel planning in Phase 5

[build-dependencies]
godot-bindings = { git = "https://github.com/godot-rust/gdext", branch = "master" }
```

3. Create `.gdextension` file:
```ini
[configuration]
entry_symbol = "gdplanningai_rust_init"
compatibility_minimum = "4.2"

[libraries]
linux.debug.x86_64 = "res://addons/GdPlanningAI/bin/linux/libgdplanningai_rust.so"
linux.release.x86_64 = "res://addons/GdPlanningAI/bin/linux/libgdplanningai_rust.so"
windows.debug.x86_64 = "res://addons/GdPlanningAI/bin/windows/gdplanningai_rust.dll"
windows.release.x86_64 = "res://addons/GdPlanningAI/bin/windows/gdplanningai_rust.dll"
macos.debug = "res://addons/GdPlanningAI/bin/macos/libgdplanningai_rust.dylib"
macos.release = "res://addons/GdPlanningAI/bin/macos/libgdplanningai_rust.dylib"
```

4. Create `src/lib.rs`:
```rust
use godot::prelude::*;

mod planning_engine;
mod blackboard;
mod action;
mod precondition;
mod goal;
mod bridge;

struct GdPlanningAIExt;

#[gdextension]
unsafe impl ExtensionLibrary for GdPlanningAIExt {
    fn on_level_init(level: InitLevel) {
        match level {
            InitLevel::Scene => {
                // Register classes
                planning_engine::register_classes();
            }
            _ => {}
        }
    }
}
```

**Verification**:
- [ ] Rust project compiles successfully
- [ ] GDExtension loads in Godot project
- [ ] No errors in Godot console on startup
- [ ] Can call basic Rust function from GDScript

**Success Criteria**:
- Godot project runs with Rust extension loaded
- Basic "hello world" test passes between Rust and GDScript

---

### Step 1.2: Core Data Structures

**Objective**: Implement Rust data structures that mirror GDScript types

**Actions**:

1. Create `src/blackboard.rs`:
```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

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
    Vector2 { x: f64, y: f64 },
    Vector3 { x: f64, y: f64, z: f64 },
    Null,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ObjectDataSnapshot {
    pub uid: String,
    pub groups: Vec<String>,
    pub properties: HashMap<String, PropertyValue>,
    pub position: Option<Vector3>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Vector3 {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Vector3 {
    pub fn length(&self) -> f64 {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

impl BlackboardState {
    pub fn get_property(&self, key: &str) -> Option<&PropertyValue> {
        self.properties.get(key)
    }
    
    pub fn set_property(&mut self, key: String, value: PropertyValue) {
        self.properties.insert(key, value);
    }
    
    pub fn get_object_by_uid(&self, uid: &str) -> Option<&ObjectDataSnapshot> {
        self.objects.iter().find(|obj| obj.uid == uid)
    }
    
    pub fn get_objects_in_group(&self, group: &str) -> Vec<&ObjectDataSnapshot> {
        self.objects.iter()
            .filter(|obj| obj.groups.contains(&group.to_string()))
            .collect()
    }
}
```

2. Create `src/precondition.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::blackboard::{BlackboardState, PropertyValue};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PreconditionDef {
    pub target: PreconditionTarget,
    pub operation: PreconditionOp,
    pub property_name: String,
    pub value: Option<PropertyValue>,
    pub is_satisfied: bool,
    pub precondition_id: Option<String>, // For custom callbacks
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreconditionTarget {
    Agent,
    WorldState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PreconditionOp {
    // Built-in operations (handled in Rust)
    HasProperty,
    Equal,
    NotEqual,
    GreaterThan,
    GreaterThanOrEqual,
    LessThan,
    LessThanOrEqual,
    
    // Custom operations (callback to GDScript)
    CustomCallback,
}

impl PreconditionDef {
    pub fn evaluate(
        &self,
        agent_blackboard: &BlackboardState,
        world_state: &BlackboardState,
    ) -> PreconditionResult {
        if self.is_satisfied {
            return PreconditionResult::Satisfied;
        }
        
        // Custom callbacks are handled by the bridge
        if matches!(self.operation, PreconditionOp::CustomCallback) {
            return PreconditionResult::NeedsCallback(
                self.precondition_id.clone().unwrap()
            );
        }
        
        // Built-in operations
        let source = match self.target {
            PreconditionTarget::Agent => agent_blackboard,
            PreconditionTarget::WorldState => world_state,
        };
        
        let result = match &self.operation {
            PreconditionOp::HasProperty => {
                source.properties.contains_key(&self.property_name)
            }
            PreconditionOp::GreaterThan => {
                let prop = source.get_property(&self.property_name);
                let val = self.value.as_ref();
                Self::compare_greater_than(prop, val)
            }
            // ... implement other operations
            _ => false,
        };
        
        if result {
            PreconditionResult::Satisfied
        } else {
            PreconditionResult::NotSatisfied
        }
    }
    
    fn compare_greater_than(
        prop: Option<&PropertyValue>,
        val: Option<&PropertyValue>,
    ) -> bool {
        match (prop, val) {
            (Some(PropertyValue::Float(p)), Some(PropertyValue::Float(v))) => p > v,
            (Some(PropertyValue::Int(p)), Some(PropertyValue::Int(v))) => p > v,
            _ => false,
        }
    }
}

pub enum PreconditionResult {
    Satisfied,
    NotSatisfied,
    NeedsCallback(String), // ID for callback
}
```

3. Create `src/action.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::blackboard::BlackboardState;
use super::precondition::PreconditionDef;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ActionDefinition {
    pub uid: String,
    pub action_type: ActionType,
    pub cost: f64,
    pub preconditions: Vec<PreconditionDef>,
    pub validity_checks: Vec<PreconditionDef>,
    pub has_custom_effect: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ActionType {
    SelfAction,
    SpatialAction {
        target_object_uid: String,
    },
}

impl ActionDefinition {
    pub fn simulate_effect(
        &self,
        agent_blackboard: &mut BlackboardState,
        world_state: &mut BlackboardState,
    ) -> EffectResult {
        if self.has_custom_effect {
            // Callback to GDScript
            EffectResult::NeedsCallback(self.uid.clone())
        } else {
            // Built-in effects (if any)
            EffectResult::Applied
        }
    }
    
    pub fn reverse_simulate_effect(
        &self,
        agent_blackboard: &mut BlackboardState,
        world_state: &mut BlackboardState,
    ) -> EffectResult {
        // Most actions don't need reverse simulation
        EffectResult::Applied
    }
}

pub enum EffectResult {
    Applied,
    NeedsCallback(String), // Action UID for callback
}
```

4. Create `src/goal.rs`:
```rust
use serde::{Deserialize, Serialize};
use super::precondition::PreconditionDef;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GoalDefinition {
    pub uid: String,
    pub reward: f64,
    pub desired_state: Vec<PreconditionDef>,
}
```

**Verification**:
- [ ] All data structures compile
- [ ] Serde serialization/deserialization works
- [ ] Unit tests pass for each structure
- [ ] Can serialize from GDScript dictionary format

**Test Cases**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_blackboard_property_access() {
        let mut bb = BlackboardState::default();
        bb.set_property("hunger".to_string(), PropertyValue::Float(50.0));
        
        assert_eq!(bb.get_property("hunger"), Some(&PropertyValue::Float(50.0)));
    }
    
    #[test]
    fn test_precondition_greater_than() {
        let mut bb = BlackboardState::default();
        bb.set_property("hunger".to_string(), PropertyValue::Float(75.0));
        
        let precondition = PreconditionDef {
            target: PreconditionTarget::Agent,
            operation: PreconditionOp::GreaterThan,
            property_name: "hunger".to_string(),
            value: Some(PropertyValue::Float(50.0)),
            is_satisfied: false,
            precondition_id: None,
        };
        
        let result = precondition.evaluate(&bb, &BlackboardState::default());
        assert!(matches!(result, PreconditionResult::Satisfied));
    }
}
```

---

### Step 1.3: Callback Mechanism Design

**Objective**: Design the callback mechanism for custom preconditions and effects

**Actions**:

1. Create `src/bridge.rs`:
```rust
use godot::prelude::*;
use super::blackboard::BlackboardState;

pub struct BridgeInterface {
    // Callback functions registered from GDScript
    evaluate_precondition_callback: Option<Callable>,
    simulate_effect_callback: Option<Callable>,
}

impl BridgeInterface {
    pub fn new() -> Self {
        Self {
            evaluate_precondition_callback: None,
            simulate_effect_callback: None,
        }
    }
    
    pub fn register_precondition_callback(&mut self, callback: Callable) {
        self.evaluate_precondition_callback = Some(callback);
    }
    
    pub fn register_effect_callback(&mut self, callback: Callable) {
        self.simulate_effect_callback = Some(callback);
    }
    
    pub fn evaluate_custom_precondition(
        &self,
        precondition_id: &str,
        agent_blackboard: &BlackboardState,
        world_state: &BlackboardState,
    ) -> bool {
        if let Some(callback) = &self.evaluate_precondition_callback {
            // Convert blackboards to dictionaries
            let agent_dict = blackboard_to_dict(agent_blackboard);
            let world_dict = blackboard_to_dict(world_state);
            
            // Call GDScript
            let result = callback.call(&[
                precondition_id.to_variant(),
                agent_dict.to_variant(),
                world_dict.to_variant(),
            ]);
            
            result.try_to::<bool>().unwrap_or(false)
        } else {
            false
        }
    }
    
    pub fn simulate_custom_effect(
        &self,
        action_id: &str,
        agent_blackboard: &mut BlackboardState,
        world_state: &mut BlackboardState,
    ) {
        if let Some(callback) = &self.simulate_effect_callback {
            // Convert blackboards to dictionaries
            let mut agent_dict = blackboard_to_dict(agent_blackboard);
            let mut world_dict = blackboard_to_dict(world_state);
            
            // Call GDScript (modifies dictionaries in place)
            callback.call(&[
                action_id.to_variant(),
                agent_dict.to_variant(),
                world_dict.to_variant(),
            ]);
            
            // Update blackboards from modified dictionaries
            *agent_blackboard = dict_to_blackboard(&agent_dict);
            *world_state = dict_to_blackboard(&world_dict);
        }
    }
}

fn blackboard_to_dict(bb: &BlackboardState) -> Dictionary {
    // Implementation: convert Rust struct to Godot Dictionary
    // ...
}

fn dict_to_blackboard(dict: &Dictionary) -> BlackboardState {
    // Implementation: convert Godot Dictionary to Rust struct
    // ...
}
```

**Verification**:
- [ ] Callback registration works
- [ ] Can call GDScript function from Rust
- [ ] Can pass data back and forth
- [ ] Modifications to blackboard persist

---

## Phase 2: Core Engine (Weeks 3-4)

### Step 2.1: Planning Algorithm Implementation

**Objective**: Port the planning algorithm from `Plan.gd` to Rust

**Reference File**: `addons/GdPlanningAI/scripts/refcounteds/plan.gd`

**Actions**:

1. Create `src/planning_engine.rs`:
```rust
use godot::prelude::*;
use super::blackboard::BlackboardState;
use super::action::{ActionDefinition, EffectResult};
use super::precondition::{PreconditionDef, PreconditionResult};
use super::goal::GoalDefinition;
use super::bridge::BridgeInterface;

#[derive(GodotClass)]
#[class(base=RefCounted)]
pub struct RustPlanningEngine {
    max_recursion: usize,
    bridge: BridgeInterface,
    #[base]
    base: Base<RefCounted>,
}

#[godot_api]
impl RustPlanningEngine {
    #[func]
    fn build_plan(
        &mut self,
        agent_blackboard: Dictionary,
        world_state: Dictionary,
        actions: Array<Dictionary>,
        goals: Array<Dictionary>,
    ) -> Dictionary {
        // Deserialize inputs
        let agent_bb = dict_to_blackboard(&agent_blackboard);
        let world_bb = dict_to_blackboard(&world_state);
        let action_defs = actions_to_definitions(&actions);
        let goal_defs = goals_to_definitions(&goals);
        
        // Run planning algorithm
        let result = self.plan(agent_bb, world_bb, action_defs, goal_defs);
        
        // Serialize result
        result.to_dict()
    }
    
    #[func]
    fn set_max_recursion(&mut self, max: usize) {
        self.max_recursion = max;
    }
    
    #[func]
    fn register_callbacks(
        &mut self,
        precondition_callback: Callable,
        effect_callback: Callable,
    ) {
        self.bridge.register_precondition_callback(precondition_callback);
        self.bridge.register_effect_callback(effect_callback);
    }
}

impl RustPlanningEngine {
    fn plan(
        &mut self,
        agent_blackboard: BlackboardState,
        world_state: BlackboardState,
        actions: Vec<ActionDefinition>,
        goals: Vec<GoalDefinition>,
    ) -> PlanResult {
        // Sort goals by reward (descending)
        let mut sorted_goals = goals;
        sorted_goals.sort_by(|a, b| b.reward.partial_cmp(&a.reward).unwrap());
        
        // Try each goal until one succeeds
        for goal in sorted_goals {
            let result = self.try_build_plan_for_goal(
                agent_blackboard.clone(),
                world_state.clone(),
                &actions,
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
        let mut root_node = PlanTreeNode {
            action_uid: "root".to_string(),
            cost: 0.0,
            desired_state: goal.desired_state.clone(),
            children: vec![],
        };
        
        let success = self.build_plan_recursive(
            &mut root_node,
            vec![],
            agent_blackboard,
            world_state,
            actions,
            0,
        );
        
        if success {
            let plan = self.extract_best_plan(&root_node);
            PlanResult {
                success: true,
                action_chain: plan.actions,
                total_cost: plan.cost,
                plan_tree: Some(root_node),
            }
        } else {
            PlanResult::failure()
        }
    }
    
    fn build_plan_recursive(
        &mut self,
        node: &mut PlanTreeNode,
        prior_actions: Vec<String>,
        blackboard: BlackboardState,
        world_state: BlackboardState,
        actions: &[ActionDefinition],
        recursion_level: usize,
    ) -> bool {
        // 1. Check recursion depth
        if recursion_level > self.max_recursion {
            return false;
        }
        
        // 2. Check if goal satisfied
        if self.is_goal_satisfied(&node.desired_state, &blackboard, &world_state) {
            return true;
        }
        
        // 3. Try each action
        let mut has_solution = false;
        
        for action in actions {
            // Clone states for simulation
            let mut sim_blackboard = blackboard.clone();
            let mut sim_world_state = world_state.clone();
            
            // Calculate cost
            let cost = action.cost;
            if cost == f64::INFINITY {
                continue;
            }
            
            // Simulate effect (may callback to GDScript)
            let effect_result = action.simulate_effect(
                &mut sim_blackboard,
                &mut sim_world_state,
            );
            
            if let EffectResult::NeedsCallback(action_id) = effect_result {
                self.bridge.simulate_custom_effect(
                    &action_id,
                    &mut sim_blackboard,
                    &mut sim_world_state,
                );
            }
            
            // Backpropagate with prior actions
            for prior_uid in &prior_actions {
                if let Some(prior_action) = actions.iter().find(|a| &a.uid == prior_uid) {
                    prior_action.reverse_simulate_effect(
                        &mut sim_blackboard,
                        &mut sim_world_state,
                    );
                }
            }
            
            // Evaluate if action makes progress toward goal
            let should_use_action = self.evaluate_goals(
                &node.desired_state,
                &sim_blackboard,
                &sim_world_state,
            );
            
            if should_use_action {
                // Create next node
                let mut next_node = PlanTreeNode {
                    action_uid: action.uid.clone(),
                    cost,
                    desired_state: node.desired_state.clone(),
                    children: vec![],
                };
                
                // Add action's preconditions
                next_node.desired_state.extend(action.preconditions.clone());
                
                // Recurse
                let mut new_prior = prior_actions.clone();
                new_prior.push(action.uid.clone());
                
                if self.build_plan_recursive(
                    &mut next_node,
                    new_prior,
                    sim_blackboard,
                    sim_world_state,
                    actions,
                    recursion_level + 1,
                ) {
                    node.children.push(next_node);
                    has_solution = true;
                }
            }
        }
        
        has_solution
    }
    
    fn evaluate_goals(
        &mut self,
        preconditions: &[PreconditionDef],
        blackboard: &BlackboardState,
        world_state: &BlackboardState,
    ) -> bool {
        let mut is_closer_to_goal = false;
        
        for precondition in preconditions {
            if precondition.is_satisfied {
                continue;
            }
            
            let result = precondition.evaluate(blackboard, world_state);
            
            match result {
                PreconditionResult::Satisfied => {
                    is_closer_to_goal = true;
                }
                PreconditionResult::NeedsCallback(id) => {
                    let callback_result = self.bridge.evaluate_custom_precondition(
                        &id,
                        blackboard,
                        world_state,
                    );
                    if callback_result {
                        is_closer_to_goal = true;
                    }
                }
                PreconditionResult::NotSatisfied => {}
            }
        }
        
        is_closer_to_goal
    }
    
    fn is_goal_satisfied(
        &self,
        preconditions: &[PreconditionDef],
        blackboard: &BlackboardState,
        world_state: &BlackboardState,
    ) -> bool {
        preconditions.iter().all(|p| p.is_satisfied)
    }
    
    fn extract_best_plan(&self, root: &PlanTreeNode) -> ExtractedPlan {
        // Find the lowest-cost path through the tree
        // ...
    }
}

#[derive(Clone, Debug)]
pub struct PlanTreeNode {
    pub action_uid: String,
    pub cost: f64,
    pub desired_state: Vec<PreconditionDef>,
    pub children: Vec<PlanTreeNode>,
}

#[derive(Clone, Debug)]
pub struct PlanResult {
    pub success: bool,
    pub action_chain: Vec<String>,
    pub total_cost: f64,
    pub plan_tree: Option<PlanTreeNode>,
}

struct ExtractedPlan {
    actions: Vec<String>,
    cost: f64,
}
```

**Verification**:
- [ ] Planning algorithm produces same results as GDScript version
- [ ] Handles recursion depth correctly
- [ ] Callbacks work for custom preconditions
- [ ] Callbacks work for custom effects
- [ ] Performance is better than GDScript version

**Test Cases**:
```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_simple_plan() {
        // Create simple action/goal setup
        // Verify plan is built correctly
    }
    
    #[test]
    fn test_recursion_limit() {
        // Verify planning stops at max_recursion
    }
    
    #[test]
    fn test_goal_satisfaction() {
        // Verify goal satisfaction check works
    }
}
```

---

## Phase 3: Bridge Layer (Weeks 5-6)

### Step 3.1: GDScript Bridge Implementation

**Objective**: Implement the GDScript side of the bridge

**Actions**:

1. Create `addons/GdPlanningAI/scripts/gdpai_rust_bridge.gd`:
```gdscript
class_name GdPAIRustBridge
extends RefCounted

## Internal bridge - users don't interact with this

var planning_engine: RefCounted
var _custom_preconditions: Dictionary = {}
var _custom_actions: Dictionary = {}

func _init():
    planning_engine = RustPlanningEngine.new()
    planning_engine.register_callbacks(
        _evaluate_precondition_callback,
        _simulate_effect_callback
    )

## Called by Rust when evaluating custom preconditions
func _evaluate_precondition_callback(
    precondition_id: String,
    agent_blackboard: Dictionary,
    world_state: Dictionary
) -> bool:
    if precondition_id in _custom_preconditions:
        var callable: Callable = _custom_preconditions[precondition_id]
        var agent_bb = _dict_to_blackboard(agent_blackboard)
        var world_bb = _dict_to_blackboard(world_state)
        return callable.call(agent_bb, world_bb)
    return false

## Called by Rust when simulating custom effects
func _simulate_effect_callback(
    action_id: String,
    agent_blackboard: Dictionary,
    world_state: Dictionary
) -> void:
    if action_id in _custom_actions:
        var action: Action = _custom_actions[action_id]
        var agent_bb = _dict_to_blackboard(agent_blackboard)
        var world_bb = _dict_to_blackboard(world_state)
        action.simulate_effect(agent_bb, world_bb)

## Register user's custom precondition
func _register_precondition(callable: Callable) -> String:
    var id = str(callable.get_object_id(), "_", callable.get_method())
    _custom_preconditions[id] = callable
    return id

## Register user's action for callbacks
func _register_action(action: Action) -> void:
    _custom_actions[action.uid] = action

## Main planning entry point
func build_plan(
    agent: GdPAIAgent,
    actions: Array[Action],
    goals: Array[Goal]
) -> Dictionary:
    # Clear registries for new planning session
    _custom_preconditions.clear()
    _custom_actions.clear()
    
    # Serialize inputs
    var agent_state = _serialize_blackboard(agent.blackboard)
    var world_state = _serialize_blackboard(agent.world_node.get_world_state())
    var serialized_actions = _serialize_actions(actions)
    var serialized_goals = _serialize_goals(goals, agent)
    
    # Call Rust planning engine
    var result = planning_engine.build_plan(
        agent_state,
        world_state,
        serialized_actions,
        serialized_goals
    )
    
    return result

## Convert plan result back to action chain
func deserialize_plan_result(result: Dictionary, actions: Array[Action]) -> Array[Action]:
    var action_chain: Array[Action] = []
    for action_uid in result.action_chain:
        for action in actions:
            if action.uid == action_uid:
                action_chain.append(action)
                break
    return action_chain

## Serialization helpers
func _serialize_blackboard(blackboard: GdPAIBlackboard) -> Dictionary:
    var serialized = {
        "properties": {},
        "objects": []
    }
    
    for key in blackboard.get_dict():
        if key == GdPAIBlackboard.GDPAI_OBJECTS:
            continue
        serialized.properties[key] = _serialize_property(blackboard.get_property(key))
    
    for obj in blackboard.get_property(GdPAIBlackboard.GDPAI_OBJECTS):
        serialized.objects.append(_serialize_object_data(obj))
    
    return serialized

func _serialize_actions(actions: Array[Action]) -> Array[Dictionary]:
    var serialized = []
    for action in actions:
        _register_action(action)
        serialized.append({
            "uid": action.uid,
            "action_type": _get_action_type(action),
            "cost": 0.0,
            "preconditions": _serialize_preconditions(action.get_preconditions()),
            "validity_checks": _serialize_preconditions(action.get_validity_checks()),
            "has_custom_effect": true,
        })
    return serialized

func _serialize_preconditions(preconditions: Array[Precondition]) -> Array[Dictionary]:
    var serialized = []
    for precondition in preconditions:
        var dict = {
            "target": _get_precondition_target(precondition),
            "operation": _get_precondition_op(precondition),
            "property_name": "",
            "value": null,
            "is_satisfied": false,
            "precondition_id": null,
        }
        
        if _is_builtin_precondition(precondition):
            dict.property_name = _extract_property_name(precondition)
            dict.value = _extract_value(precondition)
        else:
            var id = _register_precondition(precondition.eval_func)
            dict.operation = "custom_callback"
            dict.precondition_id = id
        
        serialized.append(dict)
    return serialized

func _serialize_goals(goals: Array[Goal], agent: GdPAIAgent) -> Array[Dictionary]:
    var serialized = []
    for goal in goals:
        serialized.append({
            "uid": str(goal.get_instance_id()),
            "reward": goal.compute_reward(agent),
            "desired_state": _serialize_preconditions(goal.get_desired_state(agent)),
        })
    return serialized

func _dict_to_blackboard(dict: Dictionary) -> GdPAIBlackboard:
    var bb = GdPAIBlackboard.new()
    bb.set_dict(dict)
    return bb
```

**Verification**:
- [ ] Bridge can serialize/deserialize blackboards
- [ ] Bridge can serialize/deserialize actions
- [ ] Bridge can serialize/deserialize goals
- [ ] Callbacks work correctly
- [ ] Existing user actions work without modification

---

### Step 3.2: Integration with Existing Agent

**Objective**: Modify `GdPAIAgent` to use Rust bridge transparently

**Actions**:

1. Modify `addons/GdPlanningAI/scripts/nodes/gdpai_agent.gd`:
```gdscript
# Add at top of file
var _rust_bridge: GdPAIRustBridge = null
@export var use_rust_planning: bool = true

func _ready() -> void:
    # ... existing code ...
    
    # Initialize Rust bridge if available
    if use_rust_planning:
        _rust_bridge = GdPAIRustBridge.new()

func _query_world_state_and_plan() -> void:
    # Try Rust planner first
    if use_rust_planning and _rust_bridge != null:
        var result = await _rust_plan()
        if result.success:
            _apply_rust_plan(result)
            return
    
    # Fallback to GDScript planner
    var result = await _gdscript_plan()
    _apply_plan(result)

func _rust_plan() -> Dictionary:
    var worldly_actions = await _compute_worldly_actions()
    var self_actions = await _compute_valid_self_actions()
    var all_actions = self_actions + worldly_actions
    
    return _rust_bridge.build_plan(self, all_actions, goals)

func _apply_rust_plan(result: Dictionary) -> void:
    var action_chain = _rust_bridge.deserialize_plan_result(
        result,
        _get_all_actions()
    )
    
    _current_plan = _create_plan_from_actions(action_chain)
    _current_plan_step = -1
    _reset_runtime_status(action_chain)
    _update_debugger_info()
```

**Verification**:
- [ ] Agent uses Rust planner when enabled
- [ ] Falls back to GDScript when Rust fails
- [ ] Existing scenes work without modification
- [ ] Debug visualization still works

---

## Phase 4: Integration & Testing (Weeks 7-8)

### Step 4.1: Comprehensive Testing

**Objective**: Ensure behavioral parity and performance improvement

**Actions**:

1. Create test scenes comparing GDScript vs Rust:
   - `tests/rust_migration/single_agent_test.tscn`
   - `tests/rust_migration/multi_agent_test.tscn`
   - `tests/rust_migration/complex_plan_test.tscn`

2. Create benchmark script:
```gdscript
# tests/rust_migration/benchmark.gd
extends Node

func _ready():
    var scenarios = [
        {"name": "single_agent_simple", "agents": 1, "objects": 10},
        {"name": "multi_agent_medium", "agents": 10, "objects": 50},
        {"name": "multi_agent_complex", "agents": 50, "objects": 200},
    ]
    
    for scenario in scenarios:
        var gdscript_time = await _benchmark_gdscript(scenario)
        var rust_time = await _benchmark_rust(scenario)
        
        print("Scenario: %s" % scenario.name)
        print("  GDScript: %.2f ms" % gdscript_time)
        print("  Rust: %.2f ms" % rust_time)
        print("  Speedup: %.2fx" % (gdscript_time / rust_time))
        print("")
```

3. Test with existing example scenes:
   - `examples/2D/single_agent_demo.tscn`
   - `examples/2D/multi_agent_demo.tscn`
   - `examples/hunger/` scenes
   - `examples/fruit_tree/` scenes

**Verification**:
- [ ] All existing demo scenes work with Rust planner
- [ ] Behavioral parity confirmed (same plans produced)
- [ ] Performance improvement measured (target: 2x speedup)
- [ ] No crashes or errors in production scenarios

---

### Step 4.2: Debug Visualization Integration

**Objective**: Ensure debugging still works with Rust planner

**Actions**:

1. Verify debug tree serialization works:
```rust
// In planning_engine.rs
pub struct PlanResult {
    // ...
    pub plan_tree: Option<PlanTreeNode>,
}

impl PlanTreeNode {
    pub fn to_dict(&self) -> Dictionary {
        let dict = Dictionary::new();
        dict.set("id", self.action_uid.to_variant());
        dict.set("cost", self.cost.to_variant());
        
        let children = Array::new();
        for child in &self.children {
            children.push(child.to_dict().to_variant());
        }
        dict.set("children", children.to_variant());
        
        dict
    }
}
```

2. Test debugger tab with Rust planner:
   - Plan tree displays correctly
   - Step-through works
   - Runtime status updates

**Verification**:
- [ ] Debug visualization works with Rust planner
- [ ] Plan tree shows correct structure
- [ ] Can step through plan execution

---

## Phase 5: Optimization (Weeks 9-10)

### Step 5.1: Performance Optimization

**Objective**: Maximize performance gains

**Actions**:

1. Profile Rust implementation:
```bash
cargo flamegraph --root
```

2. Optimize hot paths:
   - Blackboard cloning
   - Precondition evaluation
   - Tree traversal

3. Implement parallel planning with Rayon:
```rust
use rayon::prelude::*;

fn plan_parallel(
    &mut self,
    goals: Vec<GoalDefinition>,
    // ...
) -> PlanResult {
    // Try goals in parallel
    let results: Vec<_> = goals.par_iter()
        .map(|goal| {
            self.try_build_plan_for_goal(/* ... */)
        })
        .collect();
    
    // Return first successful plan
    for result in results {
        if result.success {
            return result;
        }
    }
    
    PlanResult::failure()
}
```

**Verification**:
- [ ] Profiling identifies bottlenecks
- [ ] Optimizations improve performance
- [ ] Parallel planning works correctly
- [ ] No race conditions or memory issues

---

## Success Checklist

### User Experience
- [ ] Users write actions/goals in GDScript (no Rust knowledge needed)
- [ ] Existing user code works without modification
- [ ] Script templates still work
- [ ] Documentation is clear for users

### Technical
- [ ] Rust planner produces same results as GDScript
- [ ] Performance improvement achieved (2x speedup)
- [ ] Debug visualization works
- [ ] Fallback mechanism works
- [ ] All platforms supported (Linux, Windows, macOS)

### Architecture
- [ ] Clean separation between Rust and GDScript
- [ ] Callback mechanism works reliably
- [ ] Serialization is efficient
- [ ] Memory management is correct (no leaks)

---

## Rollback Plan

If critical issues arise:

1. **Immediate**: Set `use_rust_planning = false` in agent config
2. **Code**: Remove Rust bridge integration code
3. **Files**: Delete `rust/` directory and `.gdextension` file
4. **Verification**: Confirm GDScript planner works

---

## Future Enhancements

After successful migration:

1. **Parallel Planning**: Use Rayon for multi-agent scenarios
2. **Incremental Planning**: Reuse previous plan fragments
3. **ML Integration**: Cost prediction with machine learning
4. **Multi-Language Support**: C#, VisualScript via same callback interface

---

## Contact & Support

For questions during implementation:
- Reference: `RUST_MIGRATION_ANALYSIS.md` for architectural details
- Reference: `README.md` for user-facing documentation
- GitHub Issues: Report bugs and feature requests

---

**Document Version**: 1.0  
**Last Updated**: March 12, 2026  
**Status**: Ready for implementation
