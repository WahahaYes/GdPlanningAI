extends "res://test/shared/scene_smoke_base.gd"
## Example scenes as data: one parameterized test runs every case in [constant CASES_PATH]
## (format in its header) and reports it as [code]<slug>::<scene>.<case>[/code]. End-to-end
## cases run only when [code]GDPAI_E2E=1[/code] ([code]make test-e2e[/code]).

const CASES_PATH: String = "res://test/scenarios/scenes.cfg"


func after_each() -> void:
	Engine.time_scale = 1.0
	Engine.max_fps = 0
	await super.after_each()


func test_scene_scenario(case_id: String = use_parameters(_case_names())) -> void:
	var cases: ConfigFile = _cases()
	var section: String = case_id.get_slice("::", 1)
	var scene: Node = (load(cases.get_value(section, "scene")) as PackedScene).instantiate()
	match cases.get_value(section, "mode"):
		"settle":
			await _check_settled(case_id, cases, section, scene)
		"run":
			await _check_run(case_id, cases, section, scene)
		var mode:
			fail_test("%s: unknown mode %s" % [case_id, mode])


## Case names to run: the end-to-end ones only when asked for.
func _case_names() -> Array:
	var cases: ConfigFile = _cases()
	var e2e: bool = OS.get_environment("GDPAI_E2E") == "1"
	var names: Array = []
	for section in cases.get_sections():
		if cases.get_value(section, "e2e", false) == e2e:
			names.append("%s::%s" % [cases.get_value(section, "slug"), section])
	return names


func _cases() -> ConfigFile:
	var cases: ConfigFile = ConfigFile.new()
	if cases.load(CASES_PATH) != OK:
		push_error("cannot read %s" % CASES_PATH)
	return cases


## Parks agents, settles, and checks the scene kept its nodes.
func _check_settled(case_id: String, cases: ConfigFile, section: String, scene: Node) -> void:
	await _settle_example(scene)
	for path in cases.get_value(section, "nodes", []):
		assert_not_null(scene.get_node_or_null(path), "%s: keeps %s" % [case_id, path])
	var min_counts: Dictionary = cases.get_value(section, "min_counts", {})
	for cls in min_counts:
		var count: int = scene.find_children("*", cls, true, false).size()
		assert_gte(count, min_counts[cls], "%s: at least %d %s" % [case_id, min_counts[cls], cls])


## Runs the scene with agents planning, then checks what they did.
func _check_run(case_id: String, cases: ConfigFile, section: String, scene: Node) -> void:
	var required: Array = cases.get_value(section, "completes", [])
	var observe: bool = cases.get_value(section, "observe", false)
	add_child_autofree(scene)
	await _pump_frames(2)
	var agents: Array[Node] = scene.find_children("*", "GdPAIAgent", true, false)
	assert_gt(agents.size(), 0, "%s: scene has agents" % case_id)
	if agents.is_empty():
		return
	if cases.has_section_key(section, "hunger"):
		for agent: GdPAIAgent in agents:
			agent.blackboard.set_property("hunger", cases.get_value(section, "hunger"))
	Engine.time_scale = cases.get_value(section, "time_scale", 1.0)
	if observe:
		Engine.max_fps = 60

	var stats: Dictionary = {}  # agent -> per-agent stats
	var completed: Dictionary = {}  # action title -> true
	var fires: Dictionary = {}  # CampfireObject -> fuel seen in the second half
	var seconds: float = cases.get_value(section, "seconds")
	var start: int = Time.get_ticks_msec()
	var deadline: int = start + int(seconds * 1000.0)
	while Time.get_ticks_msec() < deadline:
		await get_tree().process_frame
		for agent: GdPAIAgent in agents:
			_track(agent, stats, completed)
		if observe and Time.get_ticks_msec() > start + int(seconds * 500.0):
			for fire in scene.find_children("*", "CampfireObject", true, false):
				fires[fire] = fires.get(fire, false) or fire.current_fuel > 0.0
		if not observe and required.all(func(t: String) -> bool: return completed.has(t)):
			break

	_accept_warnings()
	var missing: Array = required.filter(func(t: String) -> bool: return not completed.has(t))
	assert_eq(missing, [], "%s: every listed action completes" % case_id)
	if not observe:
		return
	_report(case_id, agents, stats)
	if cases.has_section_key(section, "max_hunger_below"):
		var limit: float = cases.get_value(section, "max_hunger_below")
		for agent: GdPAIAgent in agents:
			assert_lt(
				stats[agent].max_hunger, limit, "%s: %s never starves" % [case_id, _label(agent)]
			)
	if cases.has_section_key(section, "min_finished"):
		var least: int = cases.get_value(section, "min_finished")
		for agent: GdPAIAgent in agents:
			assert_gte(
				stats[agent].finished, least, "%s: %s finishes chains" % [case_id, _label(agent)]
			)
	if cases.get_value(section, "fires_burn", false):
		assert_gt(fires.size(), 0, "%s: scene has campfires" % case_id)
		for fire in fires:
			assert_true(fires[fire], "%s: %s burns in the second half" % [case_id, _label(fire)])


## Records what [param agent] did since the last frame. A step's success moves it on by one;
## the last step's success and failure both jump past the end with the cleanup phase, so a
## chain counts as finished there, and its last title as completed only if
## [code]held_item[/code] changed (every loop ends by using the held item).
func _track(agent: GdPAIAgent, stats: Dictionary, completed: Dictionary) -> void:
	if not stats.has(agent):
		stats[agent] = {"chain": null, "step": -1, "held": null, "finished": 0, "max_hunger": 0.0}
	var s: Dictionary = stats[agent]
	var chain: Array[Action] = agent.get_current_plan()
	var step: int = agent.get_current_plan_step()
	var held: Variant = agent.blackboard.get_property("held_item")
	if not is_same(chain, s.chain):
		s.chain = chain
		s.step = -1
	var prev: int = s.step
	if step != prev and prev >= 0 and prev < chain.size():
		if step == prev + 1 and step < chain.size():
			completed[chain[prev].get_title()] = true
		elif prev == chain.size() - 1 and step > prev:
			s.finished += 1
			if held != s.held:
				completed[chain[prev].get_title()] = true
	s.step = step
	s.held = held
	var hunger: Variant = agent.blackboard.get_property("hunger")
	if hunger != null:
		s.max_hunger = maxf(s.max_hunger, float(hunger))


## Marks engine warnings logged during a live run as expected; errors still fail the case. GUT
## counts the addon's warnings (Rust [code]godot_warn![/code]) as engine errors, and live agents
## warn by design, e.g. when another agent frees an item a search still refers to.
func _accept_warnings() -> void:
	for error in gut.error_tracker.get_errors_for_test():
		if error.error_type == Logger.ERROR_TYPE_WARNING:
			error.handled = true


## Prints per-agent results of an observed run, for calibrating its expectations.
func _report(case_id: String, agents: Array[Node], stats: Dictionary) -> void:
	for agent: GdPAIAgent in agents:
		var s: Dictionary = stats[agent]
		gut.p(
			(
				"%s: %s finished %d, max hunger %.0f"
				% [case_id, _label(agent), s.finished, s.max_hunger]
			)
		)


func _label(node: Node) -> String:
	return String(node.get_parent().name) if node.get_parent() else String(node.name)
