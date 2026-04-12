class_name HoldableObject
extends GdPAIObjectData
## World object representing an item that can be picked up into an agent's held slot.
##[br]
##[br]
## Demonstrates a reusable object-side contract for anything that should feed the
## shared [code]held_item[/code] blackboard flow.


## Item id stored in the agent's [code]held_item[/code] blackboard property when picked up.
@export var item_id: String = ""
## Reference to the [GdPAIInteractable] component on this object.
@export var interactable_attribs: GdPAIInteractable
## Reference to the [GdPAILocationData] component on this object.
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
