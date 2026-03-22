extends Node2D
## Integration test for the Rust bridge using the hunger example classes.
## Tests the full planning pipeline from GDScript through Rust and back.

var test_passed: bool = false
var test_message: String = ""

# Test components
var agent: GdPAIAgent
var food_object: SampleFoodObject
var goal: SampleHungerGoal
var world_node: GdPAIWorldNode


func _ready() -> void:
	# Run test after scene is fully loaded
	await get_tree().process_frame
	await _run_integration_test()
	_report_result()


func _run_integration_test() -> void:
	# Setup test components
	_setup_agent()
	_setup_world()
	_setup_food_object()
	_setup_goal()
	
	# Get available actions from the food object
	var actions: Array[Action] = food_object.get_provided_actions()
	print("[Test] Found %d actions from food_object" % actions.size())
	
	# Create the Rust bridge and build a plan
	var bridge: GdPAIRustBridge = GdPAIRustBridge.new()
	var goals_array: Array[Goal] = [goal]
	print("[Test] Created bridge, passing %d goals" % goals_array.size())

	print("[Test] Agent blackboard has properties: ", agent.blackboard.get_dict())
	print("[Test] World state has properties: ", agent.world_node.world_state.get_dict())

	print("[Test] Calling bridge.build_plan...")
	var result: Dictionary = bridge.build_plan(
		agent.blackboard,
		agent.world_node.get_world_state(),
		actions,
		goals_array,
		agent,
	)
	print("[Test] bridge.build_plan returned: ", result)
	
	# Verify results
	_verify_result(result)


func _setup_agent() -> void:
	# Create agent with blackboard initialized to hunger: 50.0
	agent = GdPAIAgent.new()
	agent.name = "TestAgent"
	
	# Create agent config with blackboard plan - set BEFORE add_child
	var config: GdPAIAgentConfig = GdPAIAgentConfig.new()
	config.blackboard_plan = GdPAIBlackboardPlan.new()
	config.blackboard_plan.blackboard_backend = {
		"hunger": 50.0,
		"GDPAI_OBJECTS": []
	}
	config.max_recursion = 4
	agent.config = config
	
	# Set entity reference - set BEFORE add_child
	agent.entity = agent
	
	# Now add to tree - _ready() will be called automatically
	add_child(agent)


func _setup_world() -> void:
	# Create world node with empty world state
	world_node = GdPAIWorldNode.new()
	world_node.name = "WorldNode"
	
	# Create world state blackboard plan - set BEFORE add_child
	var world_plan: GdPAIBlackboardPlan = GdPAIBlackboardPlan.new()
	world_plan.blackboard_backend = {
		"GDPAI_OBJECTS": []
	}
	world_node.blackboard_plan = world_plan
	
	# Now add to tree - _ready() will be called automatically
	add_child(world_node)
	
	# Link world node to agent
	agent.world_node = world_node


func _setup_food_object() -> void:
	# Create a food object that provides an eat action
	food_object = SampleFoodObject.new()
	food_object.name = "TestFood"
	food_object.hunger_value = 30.0
	food_object.eating_duration = 1.0
	add_child(food_object)
	
	# Create location data for the food (required by SampleFoodAction)
	var location_data: GdPAILocationData = GdPAILocationData.new()
	location_data.name = "FoodLocation"
	location_data.location_node_2d = self # Use test scene root as location
	add_child(location_data)

	# Create interactable attributes
	var interactable: GdPAIInteractable = GdPAIInteractable.new()
	interactable.name = "FoodInteractable"
	interactable.max_interaction_distance = 10.0
	add_child(interactable)
	
	# Link components to food object
	food_object.location_data = location_data
	food_object.interactable_attribs = interactable
	food_object.entity = self
	
	# Add food object to world state
	var world_objects = agent.world_node.world_state.get_property("GDPAI_OBJECTS")
	world_objects.append(food_object)
	agent.world_node.world_state.set_property("GDPAI_OBJECTS", world_objects)


func _setup_goal() -> void:
	# Create hunger goal
	goal = SampleHungerGoal.new()


func _verify_result(result: Dictionary) -> void:
	# Check success flag
	if not result.get("success", false):
		test_message = "Plan failed - no solution found. Result: %s" % str(result)
		return
	
	# Check action chain is non-empty
	var chain: Array = result.get("action_chain", [])
	if chain.is_empty():
		test_message = "Plan succeeded but action chain is empty"
		return
	
	# Check total cost is reasonable
	var cost: float = result.get("total_cost", INF)
	if cost <= 0.0 or cost == INF:
		test_message = "Invalid total cost: %f" % cost
		return
	
	test_passed = true
	test_message = "SUCCESS: Plan with %d actions, cost %f" % [chain.size(), cost]


func _report_result() -> void:
	if test_passed:
		prints("[TEST PASSED]", test_message)
	else:
		push_error("[TEST FAILED] " + test_message)
	get_tree().quit()
