class_name FoodObject
extends HoldableObject
## World object representing a pickupable food item.
##[br]
##[br]
## Demonstrates [HoldableObject]: adds food-specific simulation metadata on top
## of the shared pickup contract so the item can later be consumed by an
## agent-provided eating action.[br]
##[br]
## It adds no actions of its own: the inherited [PickupAction] does the work. What it
## contributes is the [code]Food[/code] group label. [PickupAction] checks that label
## to also provide the [code]is_food[/code] fact, and [EatHeldFoodAction] requires that
## fact, so a banana can be eaten and a plain [HoldableObject] cannot. The scene sets
## [member HoldableObject.item_id] (for example [code]banana[/code]); the amount
## restored comes from [member HungerBehaviorConfig.hunger_restored_by_item], not from
## [member hunger_value].

## How many points of hunger this item should restore when consumed.[br]
## Kept on the object so other planner-side actions can estimate the value of spawned food;
## [ShakeTreeAction] reads it to predict what a shake will yield. Eating itself uses the
## per-item table in the hunger config.
@export var hunger_value: float = 20.0


# Override
func get_group_labels() -> Array[String]:
	return ["FoodObject", "Food", "HoldableObject", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	var sim_properties: Dictionary = super()
	sim_properties["hunger_value"] = hunger_value
	return sim_properties
