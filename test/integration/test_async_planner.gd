extends GutTest

var _plan_ready: bool = false
var _last_plan_result: Dictionary = {}
var _plan_ready_count: int = 0


func before_each() -> void:
	_plan_ready = false
	_last_plan_result = {}
	_plan_ready_count = 0


func after_each() -> void:
	var scheduler: GdPAIPlanScheduler = GdPAIAutoload.get_scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()


func _on_plan_ready(result: Dictionary) -> void:
	_last_plan_result = result
	_plan_ready = true
	_plan_ready_count += 1


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
		self, agent_bb, world_bb, actions, goals, max_recursion, iteration_budget, 10
	)

	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if _plan_ready:
			return _last_plan_result
		await get_tree().process_frame

	# Timeout reached
	print("[TIMEOUT] Signaling cancellation for agent: ", self)
	if scheduler != null:
		print("[POOL STATUS] ", scheduler.get_pool_status())
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


func test_async_empty_plan_returns_failure() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard(),
		_make_blackboard(),
		[],
		[],
	)

	assert_false(result["success"], "Empty plan should return failure")
	assert_eq(result["goal_index"], -1, "Goal index should be -1 on failure")


func test_async_all_goals_satisfied() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": true, "hunger": 0.0}),
		_make_blackboard(),
		[],
		[
			{
				"name": "HaveKey",
				"reward": 5.0,
				"desired_state":
				[
					{
						"target": "agent",
						"operation": "equal",
						"property_name": "has_key",
						"value": true
					}
				]
			},
			{
				"name": "NotHungry",
				"reward": 10.0,
				"desired_state":
				[
					{
						"target": "agent",
						"operation": "less_than",
						"property_name": "hunger",
						"value": 1.0
					}
				]
			}
		],
	)

	assert_true(result["success"], "All goals satisfied should succeed")
	assert_eq(result["total_cost"], 0.0, "All goals satisfied should have zero cost")
	assert_eq(
		result["action_chain"].size(), 0, "All goals satisfied should have empty action chain"
	)
	assert_eq(result["goal_index"], 1, "Highest-reward satisfied goal should be selected")


func test_async_simple_one_action_plan() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_1: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var effect_get_key: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("has_key", true)
	var actions: Array[Dictionary] = [
		{
			"name": "GetKey",
			"cost_callable": cost_1,
			"effect_callable": effect_get_key,
			"preconditions": [],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 10.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "has_key", "value": true}]
		}
	]

	var result: Dictionary = await _submit_plan_and_wait(
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


func test_async_action_with_failed_precondition() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_1: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var effect_goal_met: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("goal_met", true)
	var actions: Array[Dictionary] = [
		{
			"name": "LockedAction",
			"cost_callable": cost_1,
			"effect_callable": effect_goal_met,
			"preconditions":
			[{"target": "agent", "operation": "equal", "property_name": "has_key", "value": true}],
			"validity_checks": []
		}
	]
	var goals: Array[Dictionary] = [
		{
			"name": "Goal",
			"reward": 10.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "goal_met", "value": true}]
		}
	]

	var result: Dictionary = await _submit_plan_and_wait(
		scheduler,
		_make_blackboard({"has_key": false}),
		_make_blackboard(),
		actions,
		goals,
	)

	assert_false(result["success"], "Should fail when preconditions are not met")
	assert_eq(result["goal_index"], -1, "Failed async plan should have goal_index -1")


func test_async_newer_submission_cancels_older_inflight_plan() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame

	var cost_10: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 10.0
	var cost_1: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var effect_get_key: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("has_key", true)
	var effect_get_food: Callable = func(a: GdPAIBlackboard, _w: GdPAIBlackboard) -> void:
		a.set_property("has_food", true)
	var first_actions: Array[Dictionary] = [
		{
			"name": "ExpensiveGetKey",
			"cost_callable": cost_10,
			"effect_callable": effect_get_key,
			"preconditions": [],
			"validity_checks": []
		}
	]
	var first_goals: Array[Dictionary] = [
		{
			"name": "HaveKey",
			"reward": 10.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "has_key", "value": true}]
		}
	]

	var second_actions: Array[Dictionary] = [
		{
			"name": "CheapGetFood",
			"cost_callable": cost_1,
			"effect_callable": effect_get_food,
			"preconditions": [],
			"validity_checks": []
		}
	]
	var second_goals: Array[Dictionary] = [
		{
			"name": "HaveFood",
			"reward": 20.0,
			"desired_state":
			[{"target": "agent", "operation": "equal", "property_name": "has_food", "value": true}]
		}
	]

	_plan_ready = false
	_last_plan_result = {}
	_plan_ready_count = 0

	(
		scheduler
		. submit_plan(
			self,
			_make_blackboard({"has_key": false, "has_food": false}),
			_make_blackboard(),
			first_actions,
			first_goals,
			100,
			20000,
			10,
		)
	)
	(
		scheduler
		. submit_plan(
			self,
			_make_blackboard({"has_key": false, "has_food": false}),
			_make_blackboard(),
			second_actions,
			second_goals,
			100,
			20000,
			10,
		)
	)

	for i in range(120):
		scheduler.process_callbacks()
		if _plan_ready:
			break
		await get_tree().process_frame

	assert_true(_plan_ready, "Newest plan submission should complete")
	assert_eq(_plan_ready_count, 1, "Only the newest plan result should be delivered")
	assert_true(_last_plan_result["success"], "Newest plan should succeed")
	assert_eq(_last_plan_result["total_cost"], 1.0, "Newest plan result should win")
	assert_eq(
		_last_plan_result["action_chain"],
		[0],
		"Newest plan should produce the second submission's single action"
	)


func test_agent_with_null_entity_skips_plan_submission() -> void:
	var agent: GdPAIAgent = GdPAIAgent.new()
	add_child_autofree(agent)
	agent.goals.append(Goal.new())
	agent.manually_start_plan()
	assert_false(agent._waiting_for_plan, "Null-entity agent should not leave a plan in flight")
	assert_eq(agent._plan_generation, 0, "Null-entity agent should not submit a plan generation")
	assert_true(
		agent._last_submitted_actions.is_empty(), "Null-entity agent should submit no actions"
	)
	assert_eq(
		agent.blackboard.get_property("GDPAI_OBJECTS"),
		[],
		"Null-entity agent should not collect agent objects",
	)
