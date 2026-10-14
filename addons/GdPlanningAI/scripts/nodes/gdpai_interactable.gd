class_name GdPAIInteractable
extends GdPAIObjectData
## World object agents can interact with. Exposes interaction limits to the planner; the addon
## does not enforce them, actions that need them read them.

## How close an agent must be to interact. 0 or less means no limit.
@export var max_interaction_distance: float = 2
## How far the object may move from its planned position. Negative means no limit.
@export var max_drift_from_plan: float = -1


# Override
func get_group_labels() -> Array[String]:
	return ["GdPAIInteractable", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	return {
		"max_interaction_distance": max_interaction_distance,
		"max_drift_from_plan": max_drift_from_plan,
	}
