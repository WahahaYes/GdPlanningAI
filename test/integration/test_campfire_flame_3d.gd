extends "res://test/shared/scene_smoke_base.gd"
## The 3D campfire's flame hides when the fire is out instead of scaling to zero.

const CAMPFIRE_PREFAB: PackedScene = preload("res://examples/source_3d/prefabs/campfire_3d.tscn")


## Out of fuel: flame hidden, scale kept non-zero (a zero scale is a singular basis).
func test_burnt_out_flame_is_hidden_not_zero_scaled() -> void:
	var campfire: Node3D = CAMPFIRE_PREFAB.instantiate() as Node3D
	add_child_autofree(campfire)
	var flame: Node3D = campfire.get_node("Fire") as Node3D
	campfire.campfire_object.current_fuel = 0.0
	await _pump_frames(2)
	assert_false(flame.visible, "A burnt-out fire should hide its flame")
	assert_ne(flame.scale, Vector3.ZERO, "Flame scale must never reach zero")
	assert_ne(flame.basis.determinant(), 0.0, "Flame basis must stay invertible")

	campfire.campfire_object.current_fuel = 50.0
	await _pump_frames(2)
	assert_true(flame.visible, "A fueled fire should show its flame")
	assert_almost_eq(flame.scale.x, 0.5, 0.05, "Flame scale follows fuel")
