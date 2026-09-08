extends "res://test/shared/scene_smoke_base.gd"
## Agent showcase loads and settles without errors.

const SHOWCASE_SCENE: PackedScene = preload("res://examples/agent_showcase_3d.tscn")


## The scene instantiates with agents and an active camera present.
func test_showcase_loads_and_settles() -> void:
	var scene: Node3D = SHOWCASE_SCENE.instantiate() as Node3D
	await _settle_example(scene)
	assert_gt(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 0, "Scene should keep agents"
	)
	var camera: Camera3D = scene.get_node_or_null("CameraRig/SpringArm3D/Camera3D") as Camera3D
	assert_not_null(camera, "Scene should keep its rig camera")
	assert_true(camera.current, "Rig camera should stay active")
