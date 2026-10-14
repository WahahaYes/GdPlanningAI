class_name HoldableObject
extends GdPAIObjectData
## An item that can be picked up into the agent's [code]held_item[/code] slot. Offers a
## [PickupAction]; [member item_id] is what later actions match on.

## Value written to the agent's [code]held_item[/code] on pick-up.
@export var item_id: String = ""
## This item's [GdPAIInteractable].
@export var interactable_attribs: GdPAIInteractable
## This item's [GdPAILocationData].
@export var location_data: GdPAILocationData


# Override
func get_group_labels() -> Array[String]:
	return ["HoldableObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [PickupAction.new(location_data, interactable_attribs, self)]


# Override
func get_sim_properties() -> Dictionary:
	return {
		"item_id": item_id,
	}
