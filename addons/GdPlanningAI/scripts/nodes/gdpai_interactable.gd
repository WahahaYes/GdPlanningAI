class_name GdPAIInteractable
extends GdPAIObjectData
## Marks a world object that agents can interact with, and exposes interaction limits to the
## planner. Add it as a child of the object's entity (next to other [GdPAIObjectData] nodes).
## Its [method get_sim_properties] makes [member max_interaction_distance] and
## [member max_drift_from_plan] readable on the object's [SimObjectProxy] during planning, and its
## group labels include [code]GdPAIInteractable[/code] so goals and actions can find it by group.
## The addon itself does not enforce the limits; actions that need them read them.

## How close an agent must be to interact with this object, in world units. Values of 0 or less
## mean there is no distance limit.
@export var max_interaction_distance: float = 2
## The maximum distance this object may move from its planning-time position before an
## associated spatial action should fail. Negative values mean the object may move freely.
@export var max_drift_from_plan: float = -1


# Override
## Adds the group [code]GdPAIInteractable[/code] to the labels from [GdPAIObjectData].
func get_group_labels() -> Array[String]:
	return ["GdPAIInteractable", "GdPAIObjectData"]


# Override
## Exposes [code]max_interaction_distance[/code] and [code]max_drift_from_plan[/code] to the
## simulation.
func get_sim_properties() -> Dictionary:
	return {
		"max_interaction_distance": max_interaction_distance,
		"max_drift_from_plan": max_drift_from_plan,
	}
