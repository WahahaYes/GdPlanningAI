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
	var missing: Array = await _run_until_completed(scene, 60.0, ["Pick Up Item", "Eat Held Food"])
	assert_eq(missing, [], "Every loop action should complete")


## The agent keeps its fire fuelled and cooks dug potatoes to eat.
func test_campfire_agent_fuels_fire_and_eats_cooked_potato() -> void:
	var scene: Node = load("res://examples/campfire_2d.tscn").instantiate()
	var missing: Array = await _run_until_completed(
		scene, 60.0, ["Pick Up Wood", "Add Fuel", "Dig Potato", "Cook Potato", "Eat Held Food"]
	)
	assert_eq(missing, [], "Every loop action should complete")


## Runs [param scene] with every agent at [param hunger] until each title in [param required]
## completed once, or the timeout. Returns the titles that never completed.
func _run_until_completed(scene: Node, hunger: float, required: Array) -> Array:
	add_child_autofree(scene)
	await _pump_frames(2)
	var agents: Array[Node] = scene.find_children("*", "GdPAIAgent", true, false)
	assert_gt(agents.size(), 0, "Scene should have agents")
	if agents.is_empty():
		return required
	for agent: GdPAIAgent in agents:
		agent.blackboard.set_property("hunger", hunger)
	Engine.time_scale = TIME_SCALE

	var completed: Dictionary = {}
	var seen: Dictionary = {}  # agent -> [chain, step, held_item]
	var deadline: int = Time.get_ticks_msec() + int(TIMEOUT_S * 1000.0)
	while Time.get_ticks_msec() < deadline:
		await get_tree().process_frame
		for agent: GdPAIAgent in agents:
			_track(agent, seen, completed)
		if required.all(func(t): return completed.has(t)):
			break
	return required.filter(func(t): return not completed.has(t))


## Records the action [param agent] finished since the last frame. A success moves the step on
## by one. The last step's success and failure both jump past the end with the cleanup phase,
## so it counts only if [code]held_item[/code] changed: every loop ends by using the held item.
func _track(agent: GdPAIAgent, seen: Dictionary, completed: Dictionary) -> void:
	var chain: Array[Action] = agent.get_current_plan()
	var step: int = agent.get_current_plan_step()
	var held: Variant = agent.blackboard.get_property("held_item")
	var last: Array = seen.get(agent, [null, -1, held])
	if not is_same(chain, last[0]):
		last = [chain, -1, last[2]]
	var prev: int = last[1]
	if step != prev and prev >= 0 and prev < chain.size():
		var finished_last: bool = prev == chain.size() - 1 and step > prev and held != last[2]
		if (step == prev + 1 and step < chain.size()) or finished_last:
			completed[chain[prev].get_title()] = true
	seen[agent] = [chain, step, held]
