extends "res://test/shared/scene_smoke_base.gd"
## Example agents, planning normally, complete their action loops. Unlike the smoke tests the
## agents are not parked; the scene runs sped up until every required action has completed.

## Game speed while a scene runs; planning stays in wall time.
const TIME_SCALE: float = 2.0
## Wall-clock seconds a scene may take before the test fails.
const TIMEOUT_S: float = 40.0


func after_each() -> void:
	Engine.time_scale = 1.0
	await super.after_each()


## Hungry, the agent finds fruit (shaking the tree if needed), picks it up and eats it.
func test_hunger_basic_agent_eats() -> void:
	var scene: Node = load("res://examples/hunger_basic_2d.tscn").instantiate()
	var missing: Array = await _run_until_completed(
		scene, 60.0, -1.0, ["Pick Up Item", "Eat Held Food"]
	)
	assert_eq(missing, [], "Every loop action should complete")


## The agent keeps its fire fuelled and cooks dug potatoes to eat.
func test_campfire_agent_fuels_fire_and_eats_cooked_potato() -> void:
	var scene: Node = load("res://examples/campfire_2d.tscn").instantiate()
	var missing: Array = await _run_until_completed(
		scene,
		60.0,
		-1.0,
		["Pick Up Wood", "Add Fuel", "Dig Potato", "Cook Potato", "Eat Held Food"]
	)
	assert_eq(missing, [], "Every loop action should complete")


## Runs [param scene] with every agent at [param hunger] and every campfire at [param fuel]
## (negative leaves it) until each title in [param required] completed once, or the timeout.
## Returns the titles that never completed.
func _run_until_completed(scene: Node, hunger: float, fuel: float, required: Array) -> Array:
	add_child_autofree(scene)
	await _pump_frames(2)
	var agents: Array[Node] = scene.find_children("*", "GdPAIAgent", true, false)
	assert_gt(agents.size(), 0, "Scene should have agents")
	for agent: GdPAIAgent in agents:
		agent.blackboard.set_property("hunger", hunger)
	if fuel >= 0.0:
		for campfire: CampfireObject in scene.find_children("*", "CampfireObject", true, false):
			campfire.current_fuel = fuel
	Engine.time_scale = TIME_SCALE

	var completed: Dictionary = {}
	var seen: Dictionary = {}  # agent -> [chain, step]
	var deadline: int = Time.get_ticks_msec() + int(TIMEOUT_S * 1000.0)
	while Time.get_ticks_msec() < deadline:
		await get_tree().process_frame
		for agent: GdPAIAgent in agents:
			_track(agent, seen, completed)
		if required.all(func(t): return completed.has(t)):
			break
	return required.filter(func(t): return not completed.has(t))


## Records the action [param agent] finished since the last frame. A success moves the step on
## by one; the last success and the cleanup phase share a frame, so a finished chain jumps from
## its last step past the end. A jump from an earlier step is a failure.
func _track(agent: GdPAIAgent, seen: Dictionary, completed: Dictionary) -> void:
	var chain: Array[Action] = agent.get_current_plan()
	var step: int = agent.get_current_plan_step()
	var last: Array = seen.get(agent, [null, -1])
	if not is_same(chain, last[0]):
		last = [chain, -1]
	var prev: int = last[1]
	if step != prev and prev >= 0 and prev < chain.size():
		if step == prev + 1 or (step > chain.size() - 1 and prev == chain.size() - 1):
			completed[chain[prev].get_title()] = true
	seen[agent] = [chain, step]
