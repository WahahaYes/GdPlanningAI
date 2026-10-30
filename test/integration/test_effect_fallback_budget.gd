extends GutTest
## Per-scene effect-fallback budget: the agent config default, a custom budget that
## lets a fallback-only action plan, and a zero cap that drops the branch live.
## Fixture actions live here in test/ so the examples/ effect-spec lint stays green.

var _plan_ready: bool = false
var _last_plan_result: Dictionary = {}


func before_each() -> void:
	_plan_ready = false
	_last_plan_result = {}


func after_each() -> void:
	var scheduler: GdPAIPlanScheduler = GdPAIAutoload.get_scheduler()
	if scheduler != null:
		scheduler.cancel_all_jobs()


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
	fallback_budget: int,
	timeout_frames: int = 300,
) -> Dictionary:
	_plan_ready = false
	_last_plan_result = {}
	scheduler.submit_plan(self, agent_bb, world_bb, actions, goals, 100, 20000, 10, fallback_budget)

	for i in range(timeout_frames):
		scheduler.process_callbacks()
		if _plan_ready:
			return _last_plan_result
		await get_tree().process_frame

	fail_test("Timed out waiting for async plan result")
	return {"success": false, "goal_index": -1, "action_chain": [], "total_cost": 0.0}


func _fallback_fixture() -> Array:
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
	return [actions, goals]


## Marks the expected budget-trip warning as handled; errors still fail the case. GUT
## counts the addon's warnings (Rust godot_warn!) as engine errors.
func _accept_warnings() -> void:
	var warnings: Array = []
	for error in gut.error_tracker.get_errors_for_test():
		if error.error_type == Logger.ERROR_TYPE_WARNING:
			warnings.append(error)
	assert_eq(warnings.size(), 1, "only the expected budget-trip warn fires")
	for warning in warnings:
		warning.handled = true


func test_config_default_fallback_budget_is_128() -> void:
	var config: GdPAIAgentConfig = GdPAIAgentConfig.new()
	assert_eq(config.max_effect_fallbacks, 128, "Unset budget defaults to 128")


func test_custom_budget_honors_fallback_only_action() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame
	var before: Dictionary = scheduler.get_effect_dispatch_stats()

	var parts: Array = _fallback_fixture()
	var result: Dictionary = await _submit_plan_and_wait(
		scheduler, _make_blackboard({"has_key": false}), _make_blackboard(), parts[0], parts[1], 7
	)

	assert_true(result["success"], "Fallback-only action should plan under a custom budget")
	assert_eq(result["action_chain"].size(), 1, "Plan should have 1 action")
	var after: Dictionary = scheduler.get_effect_dispatch_stats()
	assert_gt(
		int(after["effect_callback_fallbacks"]) - int(before["effect_callback_fallbacks"]),
		0,
		"Fallback-only effect should count as a callback fallback"
	)
	assert_eq(
		int(after["effect_fallback_trips"]) - int(before["effect_fallback_trips"]),
		0,
		"Budget 7 should trip nothing"
	)


func test_zero_cap_drops_fallback_branch_live() -> void:
	var scheduler: GdPAIPlanScheduler = _make_scheduler()
	await get_tree().process_frame
	var before: Dictionary = scheduler.get_effect_dispatch_stats()

	var parts: Array = _fallback_fixture()
	var result: Dictionary = await _submit_plan_and_wait(
		scheduler, _make_blackboard({"has_key": false}), _make_blackboard(), parts[0], parts[1], 0
	)

	_accept_warnings()
	assert_false(result["success"], "Zero cap should drop the fallback branch")
	var after: Dictionary = scheduler.get_effect_dispatch_stats()
	assert_eq(
		int(after["effect_fallback_trips"]) - int(before["effect_fallback_trips"]),
		1,
		"Zero cap should trip the budget once"
	)
