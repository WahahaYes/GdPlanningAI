extends Node
## Runtime script for campfire prefab scene.
## Handles fuel decay and visual updates (label, primitive scaling/tinting).
## This script is attached to the prefab scene root, not to CampfireObject.

## Campfire data object whose fuel state is updated.
@export var campfire_object: CampfireObject
## Label node that displays the current fuel level.
@export var label_node: Node
## Visual node scaled to represent the current flame intensity.
@export var fire_primitive: Node


func _process(delta: float) -> void:
	if not is_instance_valid(campfire_object):
		return

	# Decay fuel
	campfire_object.current_fuel = max(
		0.0, campfire_object.current_fuel - campfire_object.fuel_decay_rate * delta
	)

	# Update label with fuel percentage
	_update_label()

	# Update fire primitive visuals
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

	if fire_primitive is Node3D:
		var prim_3d: Node3D = fire_primitive as Node3D
		prim_3d.scale = Vector3(fuel_ratio, fuel_ratio, fuel_ratio)
	elif fire_primitive is Node2D:
		var prim_2d: Node2D = fire_primitive as Node2D
		prim_2d.scale = Vector2(fuel_ratio, fuel_ratio)
