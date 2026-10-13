extends "res://test/shared/scene_smoke_base.gd"
## Campfire 3D example loads and settles without errors.

const CAMPFIRE_3D_SCENE: PackedScene = preload("res://examples/campfire_3d.tscn")


## The scene instantiates with its world, agent, and campfire present.
func test_campfire_3d_loads_and_settles() -> void:
	var scene: Node3D = CAMPFIRE_3D_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	assert_not_null(scene.get_node_or_null("GdPAIWorldNode"), "Scene should keep its world node")
	assert_gt(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 0, "Scene should keep an agent"
	)
	assert_gt(
		scene.find_children("*", "CampfireObject", true, false).size(),
		0,
		"Scene should keep its campfire"
	)


## With a single agent the guide leaves out the agent-switching line.
func test_single_agent_guide_omits_switching() -> void:
	var scene: Node3D = CAMPFIRE_3D_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	var rig: Node3D = scene.find_child("CameraRig", true, false) as Node3D
	var guide: Label = rig.get_node("ControlsGuide").get_child(0) as Label
	assert_string_contains(guide.text, "R: reset view")
	assert_false(guide.text.contains("Tab"), "One agent: no switching controls")
