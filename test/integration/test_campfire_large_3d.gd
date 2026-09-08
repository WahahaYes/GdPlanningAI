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
