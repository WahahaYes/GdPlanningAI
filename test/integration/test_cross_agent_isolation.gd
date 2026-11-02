extends GutTest
## Cross-agent isolation: two agents planning concurrently over one shared world
## board. Each agent's search simulates effects that mutate world snapshot
## properties; the plans must reflect only their own goals, and the live world
## board (plus both agent boards) must be unchanged afterwards. Plain-value
## blackboards are isolated per job — only Object-valued properties escape the
## snapshot (see docs/AUTHORING_GUIDE.md Simulation), and this fixture uses
## plain values throughout.
##
## Fixture actions live here in test/ so the examples/ effect-spec lint stays green.


## Receives `_on_plan_ready` for one agent; `submit_plan` delivers results to the
## agent object, so two concurrent jobs need two distinct listeners.
class PlanListener:
	extends RefCounted
	var plan_ready: bool = false
	var last_result: Dictionary = {}

	func _on_plan_ready(result: Dictionary) -> void:
		last_result = result
		plan_ready = true


var _listener_a: PlanListener
var _listener_b: PlanListener
var _scheduler: GdPAIPlanScheduler


func before_each() -> void:
	_listener_a = PlanListener.new()
	_listener_b = PlanListener.new()
	_scheduler = null


func after_each() -> void:
	if is_instance_valid(_scheduler):
		_scheduler.cancel_all_jobs()


func _make_scheduler() -> GdPAIPlanScheduler:
	var scheduler: GdPAIPlanScheduler = GdPAIPlanScheduler.new()
	add_child_autofree(scheduler)
	return scheduler


func _make_blackboard(initial_values: Dictionary = {}) -> GdPAIBlackboard:
	var bb: GdPAIBlackboard = GdPAIBlackboard.new()
	for key in initial_values.keys():
		bb.set_property(key, initial_values[key])
	return bb


func _banner_actions() -> Array[Dictionary]:
	var cost_1: Callable = func(_a: GdPAIBlackboard, _w: GdPAIBlackboard) -> float: return 1.0
	var raise_blue: Callable = func(_a: GdPAIBlackboard, w: GdPAIBlackboard) -> void:
		w.set_property("banner", "blue")
	var raise_red: Callable = func(_a: GdPAIBlackboard, w: GdPAIBlackboard) -> void:
		w.set_property("banner", "red")
	return [
		{
			"name": "RaiseBlue",
			"cost_callable": cost_1,
			"effect_callable": raise_blue,
			"preconditions": [],
			"validity_checks": []
		},
		{
			"name": "RaiseRed",
			"cost_callable": cost_1,
			"effect_callable": raise_red,
			"preconditions": [],
			"validity_checks": []
		}
	]


func _banner_goal(color: String) -> Array[Dictionary]:
	return [
		{
			"name": "BannerIs" + color.capitalize(),
			"reward": 10.0,
			"desired_state":
			[
				{
					"target": "world_state",
					"operation": "equal",
					"property_name": "banner",
					"value": color
				}
			]
		}
	]


func test_concurrent_agents_are_isolated_over_shared_world() -> void:
	_scheduler = _make_scheduler()
	var scheduler: GdPAIPlanScheduler = _scheduler
	await get_tree().process_frame

	var actions: Array[Dictionary] = _banner_actions()
	# One live world board shared by both jobs; each search simulates setting
	# `banner` to its own color, which must never reach this board.
	var world_bb: GdPAIBlackboard = _make_blackboard({"banner": "white"})
	var agent_bb_a: GdPAIBlackboard = _make_blackboard()
	var agent_bb_b: GdPAIBlackboard = _make_blackboard()

	# Submit both jobs before pumping callbacks so the searches overlap.
	scheduler.submit_plan(
		_listener_a, agent_bb_a, world_bb, actions, _banner_goal("blue"), 100, 20000, 10, 128
	)
	scheduler.submit_plan(
		_listener_b, agent_bb_b, world_bb, actions, _banner_goal("red"), 100, 20000, 10, 128
	)

	var deadline: int = 300
	for i in range(deadline):
		scheduler.process_callbacks()
		if _listener_a.plan_ready and _listener_b.plan_ready:
			break
		await get_tree().process_frame

	assert_true(_listener_a.plan_ready, "Agent A receives its plan")
	assert_true(_listener_b.plan_ready, "Agent B receives its plan")
	assert_true(_listener_a.last_result["success"], "Agent A plans BannerIsBlue")
	assert_true(_listener_b.last_result["success"], "Agent B plans BannerIsRed")
	assert_eq(_listener_a.last_result["action_chain"].size(), 1, "Agent A plan has 1 action")
	assert_eq(_listener_a.last_result["action_chain"][0], 0, "Agent A raises blue, not red")
	assert_eq(_listener_b.last_result["action_chain"].size(), 1, "Agent B plan has 1 action")
	assert_eq(_listener_b.last_result["action_chain"][0], 1, "Agent B raises red, not blue")

	# Simulation writes stayed inside the snapshots: the live boards are untouched.
	assert_eq(world_bb.get_property("banner"), "white", "Shared world board keeps its value")
	assert_false(agent_bb_a.has_property("banner"), "Agent A board gains no world keys")
	assert_false(agent_bb_b.has_property("banner"), "Agent B board gains no world keys")
