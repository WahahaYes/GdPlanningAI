extends GutTest
## Integration tests for requirements/provisions-based action chaining.
##
## These tests verify that the planner correctly chains actions where:
## - One action provides a binding (provision)
## - Another action requires that binding (requirement)
##
## This is the core of Phase 5: replacing placeholder-driven simulation with
## explicit dependency declarations.

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
) -> Dictionary:
	_plan_ready = false
	_last_plan_result = {}
	scheduler.submit_plan(self, agent_bb, world_bb, actions, goals, max_recursion)

	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if _plan_ready:
			return _last_plan_result
		await get_tree().process_frame

	fail_test("Timed out waiting for async plan result")
	return {}


## Test that Pickup -> Eat chain works when:
## - Agent is hungry (hunger > 0)
## - PickupAction provides held_item = "banana"
## - EatHeldFoodAction requires held_item exists
func test_pickup_eat_chain_satisfies_hunger() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	# Create actions using the new requirements/provisions API
	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5
	var cost_pickup: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 50.0
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		print("[TEST] eat_effect running. Current hunger: ", hunger)
		# Hypothetical Progress: Since this action has a requirement for 'held_item',
		# we report its hunger reduction effect during simulation regardless of
		# whether the item is physically present in the blackboard snapshot.
		if hunger != null:
			var new_hunger = max(0.0, float(hunger) - 20.0)
			print("[TEST] Setting new hunger: ", new_hunger)
			agent.set_property("hunger", new_hunger)
			agent.set_property("held_item", "")
	var effect_pickup: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("held_item", "banana")
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
			"name": "PickupBanana",
			"cost_callable": cost_pickup,
			"effect_callable": effect_pickup,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": "banana"}]
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

	# Agent starts hungry with no held item
	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"hunger": 30.0, "held_item": ""}),
		_make_blackboard(),
		actions,
		goals,
		120,
		4  # max_recursion must allow 2 actions
	)

	assert_true(result["success"], "Plan should succeed with Pickup -> Eat chain")
	assert_eq(result["action_chain"].size(), 2, "Plan should have 2 actions")
	assert_eq(result["action_chain"][0], 1, "First action should be Pickup (index 1)")
	assert_eq(result["action_chain"][1], 0, "Second action should be Eat (index 0)")
	assert_eq(result["total_cost"], 51.5, "Total cost should be pickup + eat = 50 + 1.5")


## Test that Eat action alone fails when no held_item is available
## (because requirements are not satisfied and no predecessor provides them)
func test_eat_alone_fails_without_pickup() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	# Only Eat action, no Pickup to provide held_item
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

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"hunger": 30.0, "held_item": ""}),
		_make_blackboard(),
		actions,
		goals,
		60,
		2
	)

	assert_false(result["success"], "Plan should fail when Eat has no way to get held_item")


## Test that Pickup -> Eat produces correct execution order
## Eat requires held_item, Pickup provides held_item
## Correct plan: Pickup first, then Eat (not Eat -> Pickup)
func test_action_order_is_pickup_then_eat_not_reversed() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5
	var cost_pickup: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 2.0
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		# Action-Led Hypothetical Progress: Report effect even if requirements are not yet physically met.
		if hunger != null:
			agent.set_property("hunger", max(0.0, float(hunger) - 20.0))
			agent.set_property("held_item", "")
	var effect_pickup: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("held_item", "banana")

	# Explicitly put Eat at index 0, Pickup at index 1
	# Forward planner might pick Eat first; backward planner should pick Pickup first
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
			"name": "PickupBanana",
			"cost_callable": cost_pickup,
			"effect_callable": effect_pickup,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": "banana"}]
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
		4
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["action_chain"].size(), 2, "Plan should have 2 actions")
	# CRITICAL: Pickup (index 1) must be FIRST, Eat (index 0) must be SECOND
	# Execution order: Pickup -> Eat (not Eat -> Pickup)
	assert_eq(result["action_chain"][0], 1, "First action MUST be Pickup (backward from goal)")
	assert_eq(result["action_chain"][1], 0, "Second action MUST be Eat (requires held_item)")


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


func test_requirement_dependent_effect_uses_provider_bound_resimulation() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5
	var cost_pickup: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 2.0
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		# Action-Led Hypothetical Progress: Report effect even if requirements are not yet physically met.
		if hunger != null:
			agent.set_property("hunger", max(0.0, float(hunger) - 20.0))
			agent.set_property("held_item", "")
	var pickup_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("held_item", "banana")

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
			"name": "PickupBanana",
			"cost_callable": cost_pickup,
			"effect_callable": pickup_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "held_item", "value": "banana"}]
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
		4
	)

	assert_true(result["success"], "Plan should re-simulate Eat after Pickup binds banana")
	assert_eq(result["action_chain"], [1, 0], "Plan should pickup banana before eating it")


func test_search_returns_cheapest_valid_requirement_chain() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_use_expensive: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 1.0
	var cost_get_expensive: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 50.0
	var cost_use_cheap: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 2.0
	var cost_get_cheap: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 1.0
	var use_tool_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var tool = agent.get_property("tool")
		if tool != null and tool != "":
			agent.set_property("task_done", true)
	var get_expensive_effect: Callable = func(
		agent: GdPAIBlackboard,
		_world: GdPAIBlackboard,
	) -> void:
		agent.set_property("tool", "expensive")
	var get_cheap_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("tool", "cheap")

	var actions: Array[Dictionary] = [
		{
			"name": "UseExpensiveTool",
			"cost_callable": cost_use_expensive,
			"effect_callable": use_tool_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements":
			[{"kind": "binding_equals", "binding_name": "tool", "value": "expensive"}],
			"provisions": []
		},
		{
			"name": "GetExpensiveTool",
			"cost_callable": cost_get_expensive,
			"effect_callable": get_expensive_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "tool", "value": "expensive"}]
		},
		{
			"name": "UseCheapTool",
			"cost_callable": cost_use_cheap,
			"effect_callable": use_tool_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "binding_equals", "binding_name": "tool", "value": "cheap"}],
			"provisions": []
		},
		{
			"name": "GetCheapTool",
			"cost_callable": cost_get_cheap,
			"effect_callable": get_cheap_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [],
			"provisions": [{"kind": "binding", "binding_name": "tool", "value": "cheap"}]
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "TaskDone",
			"reward": 100.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "task_done", "value": true}]
		}
	]

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"task_done": false, "tool": ""}),
		_make_blackboard(),
		actions,
		goals,
		120,
		4
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["total_cost"], 3.0, "Plan should use the cheapest complete valid chain")
	assert_eq(result["action_chain"], [3, 2], "Plan should get and use the cheap tool")


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


func test_wildcard_fact_provision_matches_specific_requirement() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_goto: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 5.0
	var cost_interact: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 1.0
	var goto_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("at_location", true)
	var interact_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("interacted", true)

	var actions: Array[Dictionary] = [
		{
			"name": "InteractAtLocation",
			"cost_callable": cost_interact,
			"effect_callable": interact_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "fact", "fact_name": "at_target", "args": ["location_123"]}],
			"provisions": []
		},
		{
			"name": "GoToWildcard",
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
			"name": "Interacted",
			"reward": 100.0,
			"desired_state":
			[
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "interacted",
					"value": true
				}
			]
		}
	]
	var world: GdPAIBlackboard = _make_blackboard()

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler, _make_blackboard({"interacted": false}), world, actions, goals, 120, 4
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(
		result["action_chain"], [1, 0], "Plan should chain GoToWildcard -> InteractAtLocation"
	)


func test_wildcard_fact_provision_matches_multiple_requirements() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_goto: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 5.0
	var cost_interact_a: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 1.0
	var cost_interact_b: Callable = func(
		_a: GdPAIBlackboard,
		_w: GdPAIBlackboard,
	) -> float:
		return 2.0
	var goto_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("at_location", true)
	var interact_a_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("interacted_a", true)
	var interact_b_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		agent.set_property("interacted_b", true)

	var actions: Array[Dictionary] = [
		{
			"name": "InteractAtA",
			"cost_callable": cost_interact_a,
			"effect_callable": interact_a_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "fact", "fact_name": "at_target", "args": ["location_a"]}],
			"provisions": []
		},
		{
			"name": "InteractAtB",
			"cost_callable": cost_interact_b,
			"effect_callable": interact_b_effect,
			"preconditions": [],
			"validity_checks": [],
			"requirements": [{"kind": "fact", "fact_name": "at_target", "args": ["location_b"]}],
			"provisions": []
		},
		{
			"name": "GoToWildcard",
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
			"name": "InteractedA",
			"reward": 100.0,
			"desired_state":
			[
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "interacted_a",
					"value": true
				}
			]
		}
	]
	var world: GdPAIBlackboard = _make_blackboard()

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler, _make_blackboard({"interacted_a": false}), world, actions, goals, 120, 4
	)

	assert_true(
		result["success"], "Plan should succeed with wildcard satisfying location_a requirement"
	)
	assert_eq(result["action_chain"], [2, 0], "Plan should chain GoToWildcard -> InteractAtA")


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
