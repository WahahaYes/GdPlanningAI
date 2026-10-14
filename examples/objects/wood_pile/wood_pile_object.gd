class_name WoodPileObject
extends GdPAIObjectData
## World object for a wood pile: an endless source of wood for the campfire.
##[br]
##[br]
## Provides [PickUpWoodAction] and nothing else. It has no simulated properties
## because wood is never consumed: the pile is a fixed supply, so the planner has no
## quantity to track and agents never compete for the last log. That keeps the
## maintain-fire chain (Go To, Pick Up Wood, Go To, Add Fuel) predictable. The 3D prefab
## also carries a collider and a navigation obstacle so agents walk up to the pile
## instead of through it; see the 3D scene notes in docs/EXAMPLES.md.

## Interactable metadata used by actions that target this wood pile.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData


# Override
func get_group_labels() -> Array[String]:
	return ["WoodPileObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	return [PickUpWoodAction.new(location_data, interactable_attribs)]


# Override
func get_sim_properties() -> Dictionary:
	return {}
