class_name PotatoObject
extends GdPAIObjectData
## A raw potato on the ground. Offers [DigPotatoAction]; the potato spawners keep the supply
## renewed.

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
