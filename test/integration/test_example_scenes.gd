extends "res://test/shared/scene_smoke_base.gd"
## Example scene checks that need a look inside the scene; the data-driven cases live in
## res://test/scenarios/scenes.cfg.

const CAMPFIRE_3D_SCENE: PackedScene = preload("res://examples/campfire_3d.tscn")
const LARGE_SCENE: PackedScene = preload("res://examples/campfire_large_3d.tscn")
const SHOWCASE_SCENE: PackedScene = preload("res://examples/agent_showcase_3d.tscn")
const CAMPFIRE_2D_SCENE: PackedScene = preload("res://examples/campfire_2d.tscn")


## With a single agent the guide leaves out the agent-switching line.
func test_single_agent_guide_omits_switching() -> void:
	var scene: Node3D = CAMPFIRE_3D_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var rig: Node3D = scene.find_child("CameraRig", true, false) as Node3D
	var guide: Label = _guide_label(rig)
	assert_string_contains(guide.text, "R: reset view")
	assert_false(guide.text.contains("Tab"), "One agent: no switching controls")


## Tab cycling walks every agent in order and wraps; the guide names the followed agent.
func test_camera_rig_cycles_agents_and_shows_controls() -> void:
	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var rig: Node3D = scene.find_child("CameraRig", true, false) as Node3D
	var bodies: Array[Node3D] = rig.follow_candidates()
	assert_gt(bodies.size(), 1, "Large scene should offer several agents to follow")
	var start: Node3D = rig.target
	assert_true(bodies.has(start), "Rig should start on a spawned agent")

	rig.cycle_target(1)
	assert_ne(rig.target, start, "Tab should move to another agent")
	for i in range(bodies.size() - 1):
		rig.cycle_target(1)
	assert_eq(rig.target, start, "Cycling through every agent should wrap to the start")
	rig.cycle_target(-1)
	assert_eq(
		rig.target, bodies[posmod(bodies.find(start) - 1, bodies.size())], "Shift+Tab steps back"
	)

	var guide: Label = _guide_label(rig)
	assert_string_contains(guide.text, "Following: %s" % rig.target.name)
	assert_string_contains(guide.text, "Tab")
	assert_string_contains(guide.text, "H: hide debug text")

	# One switch hides the guide and every label, including ones spawned afterwards.
	rig.show_debug_text = false
	var late: Label3D = Label3D.new()
	late.add_to_group(DebugText.GROUP)
	scene.add_child(late)
	var debug_nodes: Array[Node] = get_tree().get_nodes_in_group(DebugText.GROUP)
	assert_gt(debug_nodes.size(), 6, "Guide plus agent and object labels")
	for node in debug_nodes:
		assert_false(node.get("visible"), "%s should hide" % node.name)
	DebugText.toggle(get_tree())
	for node in debug_nodes:
		assert_true(node.get("visible"), "%s should show again" % node.name)


## Each campfire is home to exactly two agents, and every agent tends its own home fire.
func test_agents_pair_up_on_home_campfires() -> void:
	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var campfires: Array[CampfireObject] = []
	for node in GdPAIUTILS.get_children_in_group(scene, "CampfireObject"):
		var campfire: CampfireObject = node as CampfireObject
		if campfire != null:
			campfires.append(campfire)
	assert_eq(campfires.size(), 3, "Large scene keeps three campfires")
	var tenders: Dictionary = {}
	for agent_node in scene.find_children("*", "GdPAIAgent", true, false):
		for goal in (agent_node as GdPAIAgent).goals:
			if goal is MaintainFireGoal:
				var group: String = (goal as MaintainFireGoal).campfire_group
				tenders[group] = tenders.get(group, 0) + 1
	assert_eq(tenders.size(), 3, "Three home groups, no agent left on any-fire")
	for group in tenders:
		assert_eq(tenders[group], 2, "%s should have two agents" % group)
		var homes: int = 0
		for campfire in campfires:
			if campfire.is_in_group(group):
				homes += 1
		assert_eq(homes, 1, "%s should hold exactly one campfire" % group)


## The showcase's rig camera stays the active camera.
func test_showcase_camera_stays_active() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var camera: Camera3D = scene.get_node_or_null("CameraRig/SpringArm3D/Camera3D") as Camera3D
	assert_not_null(camera, "Scene should keep its rig camera")
	assert_true(camera.current, "Rig camera should stay active")


## A 2D example has the same debug-text switch: it hides the agent and object labels and
## shows them again.
func test_2d_switch_hides_labels() -> void:
	var scene: Node2D = CAMPFIRE_2D_SCENE.instantiate() as Node2D
	await _settle_example(scene)
	var switch: DebugTextSwitch = scene.get_node("DebugTextSwitch") as DebugTextSwitch
	var debug_nodes: Array[Node] = get_tree().get_nodes_in_group(DebugText.GROUP)
	assert_gt(debug_nodes.size(), 5, "Agent, campfire, potato and wood pile labels")
	switch.show_debug_text = false
	for node in debug_nodes:
		assert_false(node.get("visible"), "%s should hide" % node.name)
	DebugText.toggle(get_tree())
	for node in debug_nodes:
		assert_true(node.get("visible"), "%s should show again" % node.name)


func _guide_label(rig: Node) -> Label:
	return rig.get_node("ControlsGuide").find_children("*", "Label", true, false)[0] as Label
