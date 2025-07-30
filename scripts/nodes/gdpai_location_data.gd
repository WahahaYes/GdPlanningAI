## Node to track a relevant object's position in the world and in the agent's simulation.
class_name GdPAILocationData
extends GdPAIObjectData

@export_group("Location: only set one!")
## Location for 2D objects.
@export var location_node_2d: Node2D
## Location for 3D objects.
@export var location_node_3d: Node3D

## Shorthand way to access the entity's position, which can be altered in simulation.  Returns as
## either Vector2 or Vector3.
var position:
	get:
		assert(
			location_node_2d == null or location_node_3d == null,
			"Only one backend location node should be set!"
		)
		if location_node_2d != null:
			return location_node_2d.global_position
		elif location_node_3d != null:
			return location_node_3d.global_position
		else:
			return position
	set(val):
		position = val

## Shorthand way to access the entity's rotation (in degrees), which can be altered in simulation.
## Returns as either float (2D case) or Vector3.
var rotation:
	get:
		assert(
			location_node_2d == null or location_node_3d == null,
			"Only one backend location node should be set!"
		)
		if location_node_2d != null:
			return location_node_2d.global_rotation_degrees
		elif location_node_3d != null:
			return location_node_3d.global_rotation_degrees
		else:
			return rotation
	set(val):
		rotation = val


# Override
func get_group_labels() -> Array[String]:
	return ["GdPAILocationData", "GdPAIObjectData"]


# Override
func copy_for_simulation() -> GdPAIObjectData:
	var new_data: GdPAIObjectData = GdPAILocationData.new()
	assign_uid_and_entity(new_data)
	# NOTE: Location node is not being copied over.  At simulation time, we are essentially taking
	# 		a snapshot of position and rotation that can then be changed safely during simulation.
	new_data.position = position
	new_data.rotation = rotation
	return new_data
