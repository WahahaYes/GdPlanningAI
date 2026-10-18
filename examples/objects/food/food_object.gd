class_name FoodObject
extends HoldableObject
## A pickupable food item. Its [code]Food[/code] group makes [PickupAction] also provide
## [code]is_food[/code], which [EatHeldFoodAction] requires.

## Hunger a fruit of this kind is expected to restore; [ShakeTreeAction] uses it to predict a
## shake's value. Eating itself uses [member HungerBehaviorConfig.hunger_restored_by_item].
@export var hunger_value: float = 20.0


# Override
func get_group_labels() -> Array[String]:
	return ["FoodObject", "Food", "HoldableObject", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	var sim_properties: Dictionary = super()
	sim_properties["hunger_value"] = hunger_value
	return sim_properties
