extends Node
## Runtime script for campfire prefab scene.
## Handles fuel decay and visual updates (label, primitive scaling/tinting).
## This script is attached to the prefab scene root, not to CampfireObject.

@export var campfire_object: CampfireObject
@export var label_node: Node
@export var fire_primitive: Node


func _process(delta: float) -> void:
	if not is_instance_valid(campfire_object):
		return
	
	# Decay fuel
	campfire_object.current_fuel = max(
		0.0,
		campfire_object.current_fuel - campfire_object.fuel_decay_rate * delta
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
	
	# Scale primitive based on fuel level
	if fire_primitive.has_method("set_scale"):
		var base_scale: Vector2 = Vector2(1.0, 1.0)
		if fire_primitive is Node3D:
			var current_scale: Vector3 = fire_primitive.scale
			fire_primitive.scale = Vector3(
				current_scale.x * fuel_ratio,
				current_scale.y * fuel_ratio,
				current_scale.z * fuel_ratio
			)
		elif fire_primitive is Node2D:
			var current_scale: Vector2 = fire_primitive.scale
			fire_primitive.scale = current_scale * fuel_ratio
