class_name FoodObject
extends HoldableObject
## World object representing a pickupable food item.
##[br]
##[br]
## Demonstrates [HoldableObject]: adds food-specific simulation metadata on top
## of the shared pickup contract so the item can later be consumed by an
## agent-provided eating action.

## How many points of hunger this item should restore when consumed.
##[br]
## Kept on the object so other planner-side actions can estimate the value of spawned food.
@export var hunger_value: float = 20.0


# Override
func get_group_labels() -> Array[String]:
	return ["FoodObject", "Food", "HoldableObject", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	var sim_properties: Dictionary = super ()
	sim_properties["hunger_value"] = hunger_value
	return sim_properties
