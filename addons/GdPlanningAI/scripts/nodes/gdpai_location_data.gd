class_name GdPAILocationData
extends GdPAIObjectData
## Tracks where an object is, both in the live world and in the planner's simulation. Set exactly
## one of [member location_node_2d] and [member location_node_3d].[br]
##[br]
## Its [method get_sim_properties] snapshots [code]position[/code] and [code]rotation[/code] at the
## start of planning, so actions can read and change them on the object's [SimObjectProxy] (this is
## how [GoToAction] moves the agent in simulation). Give the agent's own entity a
## [GdPAILocationData] to make it navigable, and each destination one so it can be targeted by
## [code]at_target[/code].

@export_group("Location: only set one!")
## Node whose global transform gives the position of a 2D object.
@export var location_node_2d: Node2D
## Node whose global transform gives the position of a 3D object.
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
## Adds the group [code]GdPAILocationData[/code] to the labels from [GdPAIObjectData].
func get_group_labels() -> Array[String]:
	return ["GdPAILocationData", "GdPAIObjectData"]


# Override
## Snapshots [code]position[/code] and [code]rotation[/code]. Asserts if no location node is set.
func get_sim_properties() -> Dictionary:
	return {"position": position, "rotation": rotation}
