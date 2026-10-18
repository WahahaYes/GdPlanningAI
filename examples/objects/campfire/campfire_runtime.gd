extends Node
## Burns the campfire down and updates its visuals. Sits on the prefab root, shared by the 2D and
## 3D prefabs (hence the untyped visual nodes).[br]
## The burn-down is what makes [MaintainFireGoal] matter.

## Smallest flame scale; below it the fire is out and hidden.
const MIN_FLAME_SCALE: float = 0.01

## Campfire data whose fuel decays.
@export var campfire_object: CampfireObject
## Label showing the fuel level.
@export var label_node: Node
## Flame node, scaled by fuel.
@export var fire_primitive: Node
## Optional light, off while the fire is out.
@export var fire_light: Node


func _process(delta: float) -> void:
	if not is_instance_valid(campfire_object):
		return

	campfire_object.current_fuel = max(
		0.0, campfire_object.current_fuel - campfire_object.fuel_decay_rate * delta
	)

	_update_label()
	_update_fire_visuals()


func _update_label() -> void:
	if not is_instance_valid(label_node):
		return

	var fuel_percent: int = int(campfire_object.current_fuel)

	if label_node is Label:
		label_node.text = "Campfire (%d%%)" % fuel_percent
	elif label_node is Label3D:
		label_node.text = "Campfire (%d%%)" % fuel_percent


func _update_fire_visuals() -> void:
	if not is_instance_valid(fire_primitive):
		return

	var fuel_ratio: float = campfire_object.current_fuel / 100.0
	# Hide rather than scale to zero: a singular basis makes the renderer error every frame.
	var lit: bool = fuel_ratio > MIN_FLAME_SCALE
	var flame_scale: float = maxf(fuel_ratio, MIN_FLAME_SCALE)

	if fire_primitive is Node3D:
		var prim_3d: Node3D = fire_primitive as Node3D
		prim_3d.visible = lit
		prim_3d.scale = Vector3(flame_scale, flame_scale, flame_scale)
	elif fire_primitive is Node2D:
		var prim_2d: Node2D = fire_primitive as Node2D
		prim_2d.visible = lit
		prim_2d.scale = Vector2(flame_scale, flame_scale)

	if is_instance_valid(fire_light):
		fire_light.visible = lit
