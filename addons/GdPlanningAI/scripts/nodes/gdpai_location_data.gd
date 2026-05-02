class_name GdPAILocationData
extends GdPAIObjectData
## Node to track a relevant object's position in the world and in the agent's simulation.

@export_group("Location: only set one!")
## Location for 2D objects.
@export var location_node_2d: Node2D
## Location for 3D objects.
@export var location_node_3d: Node3D

## The entity's current position as [Vector2] (2D) or [Vector3] (3D).
var position:
	get:
		assert(
			location_node_2d == null or location_node_3d == null,
			"GdPAILocationData: both location nodes are set, only set one!"
		)
		if location_node_2d != null:
			return location_node_2d.global_position
		if location_node_3d != null:
			return location_node_3d.global_position
		assert(false, "GdPAILocationData: no location node set")

## The entity's current rotation in degrees as [float] (2D) or [Vector3] (3D).
var rotation:
	get:
		assert(
			location_node_2d == null or location_node_3d == null,
			"GdPAILocationData: both location nodes are set, only set one!"
		)
		if location_node_2d != null:
			return location_node_2d.global_rotation_degrees
		if location_node_3d != null:
			return location_node_3d.global_rotation_degrees
		assert(false, "GdPAILocationData: no location node set")


# Override
func get_group_labels() -> Array[String]:
	return ["GdPAILocationData", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	return {"position": position, "rotation": rotation}
