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
	timeout_frames: int = 120,
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
		var held_item = agent.get_property("held_item")
		if hunger != null and held_item != null and held_item != "":
			# Reduce hunger by 20 (banana value)
			agent.set_property("hunger", max(0.0, float(hunger) - 20.0))
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
		var held_item = agent.get_property("held_item")
		if hunger != null and held_item != null and held_item != "":
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


## Test that chain works when agent already has held_item
## (no Pickup needed)
func test_eat_alone_succeeds_when_already_holding_food() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_eat: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.5
	var eat_effect: Callable = func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
		var hunger = agent.get_property("hunger")
		var held_item = agent.get_property("held_item")
		if hunger != null and held_item != null and held_item != "":
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
