extends GutTest
## Requirements and provisions through `submit_plan`, for what the Rust fixtures
## (rust/tests/scenarios/) cannot show: initial provisions derived from agent
## properties, and Godot objects as bindings and fact arguments.

var _plan_ready: bool = false
var _last_plan_result: Dictionary = {}


class TestObjectData:
	extends GdPAIObjectData
	var _extra_groups: Array[String] = []

	func _init(extra_groups: Array[String] = []):
		_extra_groups = extra_groups
		super._init()

	func get_group_labels() -> Array[String]:
		var labels = super.get_group_labels()
		labels.append_array(_extra_groups)
		return labels


func before_each() -> void:
	_plan_ready = false
	_last_plan_result = {}


func _on_plan_ready(result: Dictionary) -> void:
	_last_plan_result = result
	_plan_ready = true


func _make_scheduler() -> GdPAIPlanScheduler:
	var scheduler: GdPAIPlanScheduler = GdPAIPlanScheduler.new()
	add_child_autofree(scheduler)
	return scheduler


func _make_blackboard(initial_values: Dictionary = {}) -> GdPAIBlackboard:
	var bb: GdPAIBlackboard = GdPAIBlackboard.new()
	for key in initial_values.keys():
		bb.set_property(key, initial_values[key])
	return bb


func _submit_plan_and_wait(
	scheduler: GdPAIPlanScheduler,
	agent_bb: GdPAIBlackboard,
	world_bb: GdPAIBlackboard,
	actions: Array[Dictionary],
	goals: Array[Dictionary],
	timeout_frames: int = 300,
	max_recursion: int = 100,
	iteration_budget: int = 20000,
) -> Dictionary:
	_plan_ready = false
	_last_plan_result = {}
	scheduler.submit_plan(
		self, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget, 10, 128
	)

	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if _plan_ready:
			return _last_plan_result
		await get_tree().process_frame

	# Timeout reached
	print("[TIMEOUT] Signaling cancellation for agent: ", self)
	scheduler.cancel_agent_jobs(self)

	# Give the thread a few frames to return the engine
	for i in range(10):
		scheduler.process_callbacks()
		var tree = scheduler.get_debug_tree(self)
		if tree != "" and not tree.contains("running in a background thread"):
			print("\n---------- TIMEOUT DEBUG TREE ----------")
			print(tree)
			print("----------------------------------------\n")
			break
		await get_tree().process_frame

	fail_test("Timed out waiting for async plan result")
	return {"success": false, "goal_index": -1, "action_chain": [], "total_cost": 0.0}


## Test that chain works when agent already has held_item
## (no Pickup needed)
func test_eat_alone_succeeds_when_already_holding_food() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		# Action-Led Hypothetical Progress: Report effect even if requirements are not yet physically met.
		if hunger != null:
			agent.set_property("hunger", max(0.0, float(hunger) - 20.0))
			agent.set_property("held_item", "")
	var actions: Array[Dictionary] = [
		{
			"name": "EatHeldFood",
			"cost_callable": cost_eat,
			"effect_callable": eat_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "binding_exists", "binding_name": "held_item"}],
			"provisions": []
		}
	]

	var goals: Array[Dictionary] = [
		{
			"name": "NotHungry",
			"reward": 100.0,
			"desired_state":
			[
				{
					"target": "agent",
					"operation": "less_than_or_equal",
					"property_name": "hunger",
					"value": 10.0
				}
			]
		}
	]

	# Agent already holding banana - Eat should work directly
	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"hunger": 30.0, "held_item": "banana"}),
		_make_blackboard(),
		actions,
		goals,
		60,
		2
	)

	assert_true(result["success"], "Plan should succeed when already holding food")
	assert_eq(result["action_chain"].size(), 1, "Plan should have 1 action (just Eat)")
	assert_eq(result["action_chain"][0], 0, "Action should be Eat (index 0)")
	assert_eq(result["total_cost"], 1.5, "Cost should be just eat cost")


func test_binding_in_set_requires_world_group_membership() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	var banana = TestObjectData.new(["edible"])
	var rock = TestObjectData.new(["mineral"])
	add_child_autofree(banana)
	add_child_autofree(rock)
	await get_tree().process_frame

	var cost_use: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var cost_pickup_banana: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 2.0
	var cost_pickup_rock: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 0.5
	var use_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		if agent.get_property("held_item") != null:
			agent.set_property("ate", true)
	var pickup_banana_effect: Callable = func(
		agent: GdPAIBlackboard,
		_world: GdPAIBlackboard,
	) -> void:
		agent.set_property("held_item", banana)
	var pickup_rock_effect: Callable = func(
		agent: GdPAIBlackboard,
		_world: GdPAIBlackboard,
	) -> void:
		agent.set_property("held_item", rock)

	var actions: Array[Dictionary] = [
		{
			"name": "EatEdible",
			"cost_callable": cost_use,
			"effect_callable": use_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements":
			[{"kind": "binding_in_set", "binding_name": "held_item", "set_name": "edible"}],
			"provisions": []
		},
		{
			"name": "PickupBanana",
			"cost_callable": cost_pickup_banana,
			"effect_callable": pickup_banana_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": banana}]
		},
		{
			"name": "PickupRock",
			"cost_callable": cost_pickup_rock,
			"effect_callable": pickup_rock_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": rock}]
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "Ate",
			"reward": 100.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "ate", "value": true}]
		}
	]
	var world: GdPAIBlackboard = _make_blackboard()
	world.set_property("GDPAI_OBJECTS", [banana, rock])

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler, _make_blackboard({"ate": false, "held_item": ""}), world, actions, goals, 120, 4
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["action_chain"], [1, 0], "Plan should choose the edible item provider")


## Test GoToAction with wildcard provision chains to PickupAction
## This tests using dictionary format to simulate the actual Action classes
func test_goto_action_wildcard_chains_to_pickup_interaction() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	# Create mock location data for testing
	var location_data: GdPAILocationData = GdPAILocationData.new()
	location_data.position = Vector3(10.0, 0.0, 5.0)
	add_child_autofree(location_data)

	var cost_goto: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 5.0
	var cost_pickup: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5

	var goto_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("at_location", true)
	var pickup_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("held_item", "test_item")
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		# Action-Led Hypothetical Progress: Report effect even if requirements are not yet physically met.
		if hunger != null:
			agent.set_property("hunger", max(0.0, float(hunger) - 20.0))
			agent.set_property("held_item", "")

	var actions: Array[Dictionary] = [
		{
			"name": "EatHeldFood",
			"cost_callable": cost_eat,
			"effect_callable": eat_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "binding_exists", "binding_name": "held_item"}],
			"provisions": []
		},
		{
			"name": "PickupInteraction",
			"cost_callable": cost_pickup,
			"effect_callable": pickup_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "fact", "fact_name": "at_target", "args": [location_data]}],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": "test_item"}]
		},
		{
			"name": "GoTo",
			"cost_callable": cost_goto,
			"effect_callable": goto_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "fact_wildcard", "fact_name": "at_target"}]
		}
	]

	var goals: Array[Dictionary] = [
		{
			"name": "NotHungry",
			"reward": 100.0,
			"desired_state":
			[
				{
					"target": "agent",
					"operation": "less_than_or_equal",
					"property_name": "hunger",
					"value": 10.0
				}
			]
		}
	]

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"hunger": 30.0, "held_item": ""}),
		_make_blackboard(),
		actions,
		goals,
		120,
		6
	)

	assert_true(result["success"], "Plan should succeed with GoTo -> Pickup -> Eat chain")
	# Should chain: GoTo (index 2) -> Pickup (index 1) -> Eat (index 0)
	assert_eq(result["action_chain"].size(), 3, "Plan should have 3 actions")
	assert_eq(result["action_chain"][0], 2, "First action should be GoTo")
	assert_eq(result["action_chain"][1], 1, "Second action should be Pickup")
	assert_eq(result["action_chain"][2], 0, "Third action should be Eat")
