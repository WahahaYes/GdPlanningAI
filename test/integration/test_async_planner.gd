extends GutTest

var _plan_ready := false
var _last_plan_result: Dictionary = {}


func before_each() -> void:
	_plan_ready = false
	_last_plan_result = {}


func _on_plan_ready(result: Dictionary) -> void:
	_last_plan_result = result
	_plan_ready = true


func _make_scheduler() -> GdPAIPlanScheduler:
	var scheduler := GdPAIPlanScheduler.new()
	add_child_autofree(scheduler)
	return scheduler


func _make_blackboard(initial_values: Dictionary = {}) -> GdPAIBlackboard:
	var bb := GdPAIBlackboard.new()
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
) -> Dictionary:
	_plan_ready = false
	_last_plan_result = {}
	scheduler.submit_plan(self , agent_bb, world_bb, actions, goals)

	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if _plan_ready:
			return _last_plan_result
		await get_tree().process_frame

	fail_test("Timed out waiting for async plan result")
	return {}


func test_async_empty_plan_returns_failure() -> void:
	var scheduler := _make_scheduler()
	await get_tree().process_frame

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard(),
		_make_blackboard(),
		[],
		[],
	)

	assert_false(result["success"], "Empty plan should return failure")
	assert_eq(result["goal_index"], -1, "Goal index should be -1 on failure")


func test_async_goal_already_satisfied() -> void:
	var scheduler := _make_scheduler()
	await get_tree().process_frame

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": true}),
		_make_blackboard(),
		[],
		[
			{
				"name": "HaveKey",
				"reward": 10.0,
				"desired_state": [
					{
						"target": "agent",
						"operation": "equal",
						"property_name": "has_key",
						"value": true
					}
				]
			}
		],
	)

	assert_true(result["success"], "Already satisfied goal should succeed")
	assert_eq(result["total_cost"], 0.0, "Already satisfied goal should have zero cost")
	assert_eq(
		result["action_chain"].size(),
		0,
		"Already satisfied goal should have empty action chain"
	)


func test_async_simple_one_action_plan() -> void:
	var scheduler := _make_scheduler()
	await get_tree().process_frame

	var actions: Array[Dictionary] = [
		{
			"name": "GetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			]
		}
	]

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": false}),
		_make_blackboard(),
		actions,
		goals,
	)

	assert_true(result["success"], "Simple one-action plan should succeed")
	assert_eq(result["action_chain"].size(), 1, "Plan should have 1 action")
	assert_eq(result["action_chain"][0], 0, "Plan should use the first action")
	assert_eq(result["total_cost"], 1.0, "Total cost should be 1.0")


func test_async_chooses_lower_cost_plan() -> void:
	var scheduler := _make_scheduler()
	await get_tree().process_frame

	var actions: Array[Dictionary] = [
		{
			"name": "ExpensiveGetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 10.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "CheapGetKey",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_key", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 100.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			]
		}
	]

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": false}),
		_make_blackboard(),
		actions,
		goals,
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["total_cost"], 1.0, "Should choose cheaper action with cost 1.0")
	assert_eq(result["action_chain"][0], 1, "Should choose CheapGetKey (index 1)")


func test_async_action_with_failed_precondition() -> void:
	var scheduler := _make_scheduler()
	await get_tree().process_frame

	var actions: Array[Dictionary] = [
		{
			"name": "LockedAction",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("goal_met", true),
			"preconditions": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_key",
					"value": true
				}
			],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "Goal",
			"reward": 10.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "goal_met",
					"value": true
				}
			]
		}
	]

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": false}),
		_make_blackboard(),
		actions,
		goals,
	)

	assert_false(result["success"], "Should fail when preconditions are not met")
	assert_eq(result["goal_index"], -1, "Failed async plan should have goal_index -1")


func test_async_chooses_cheaper_deeper_chain_over_direct_expensive_completion() -> void:
	var scheduler := _make_scheduler()
	scheduler.max_recursion = 3
	await get_tree().process_frame

	var actions: Array[Dictionary] = [
		{
			"name": "ExpensiveDirectCampfirePrep",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 10.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_food", true)
				agent.set_property("has_fire", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "GetFood",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_food", true),
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "LightFire",
			"cost_callable": func(_agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> float:
				return 1.0,
			"effect_callable": func(agent: GdPAIBlackboard, _world: GdPAIBlackboard) -> void:
				agent.set_property("has_fire", true),
			"preconditions": [],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "ReadyCampfireMeal",
			"reward": 100.0,
			"desired_state": [
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_food",
					"value": true
				},
				{
					"target": "agent",
					"operation": "equal",
					"property_name": "has_fire",
					"value": true
				}
			]
		}
	]

	var result := await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_food": false, "has_fire": false}),
		_make_blackboard(),
		actions,
		goals,
	)

	assert_true(result["success"], "Plan should succeed")
	assert_eq(result["total_cost"], 2.0, "Should prefer the cheaper two-step chain")
	assert_eq(result["action_chain"], [1, 2], "Should choose GetFood then LightFire")
