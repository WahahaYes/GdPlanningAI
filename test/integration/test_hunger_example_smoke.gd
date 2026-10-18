extends "res://test/shared/scene_smoke_base.gd"
## Hunger 2D example loads and settles without errors.

const HUNGER_SCENE: PackedScene = preload("res://examples/hunger_basic_2d.tscn")


## The scene instantiates with its world and agent present.
func test_hunger_loads_and_settles() -> void:
	var scene: Node2D = HUNGER_SCENE.instantiate() as Node2D
	await _settle_example(scene)
	assert_not_null(scene.get_node_or_null("GdPAIWorldNode"), "Scene should keep its world node")
	assert_gt(
		scene.find_children("*", "GdPAIAgent", true, false).size(), 0, "Scene should keep an agent"
	)
