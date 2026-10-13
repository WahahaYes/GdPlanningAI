extends "res://test/shared/scene_smoke_base.gd"
## Large campfire variant loads and settles without errors.

const LARGE_SCENE: PackedScene = preload("res://examples/campfire_large_3d.tscn")


## The scene instantiates with its world, fires, and scattered agents present.
func test_large_campfire_loads_and_settles() -> void:
	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	assert_not_null(scene.get_node_or_null("GdPAIWorldNode"), "Scene should keep its world node")
	assert_gt(
		scene.find_children("*", "CampfireObject", true, false).size(),
		0,
		"Scene should keep its campfires"
	)
	assert_gt(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 0, "Scene should keep agents"
	)


## Tab cycling walks every agent in order and wraps; the guide names the followed agent.
func test_camera_rig_cycles_agents_and_shows_controls() -> void:
	var scene: Node3D = LARGE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var rig: Node3D = scene.get_node("CameraRig") as Node3D
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

	var guide: Label = rig.get_node("ControlsGuide").get_child(0) as Label
	assert_string_contains(guide.text, "Following: %s" % rig.target.name)
	assert_string_contains(guide.text, "Tab")
