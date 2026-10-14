class_name PotatoObject
extends GdPAIObjectData
## World object for a raw potato lying on the ground.
##[br]
##[br]
## The simplest object in the examples: no simulated properties, one provided action.
## It advertises [DigPotatoAction] and the planner does the rest. The [code]Food[/code]
## group label marks it as something edible in the world's vocabulary, matching
## [FoodObject], but eating is decided by [EatHeldFoodAction]'s table of edible
## items (a raw potato is not in it; a cooked one is). Potatoes are scattered by the
## potato spawner scripts, which keep a renewable supply so the cook-and-eat
## chain never runs out of ingredients.

## Interactable metadata used by actions that target this potato.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData


# Override
func get_group_labels() -> Array[String]:
	return ["PotatoObject", "Food", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [DigPotatoAction.new(location_data, interactable_attribs, self)]


# Override
func get_sim_properties() -> Dictionary:
	return {}
