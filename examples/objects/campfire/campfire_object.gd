class_name CampfireObject
extends GdPAIObjectData
## World object for a campfire: the shared fuel tank agents keep topped up and cook at.
##[br]
##[br]
## This is the object-side half of the campfire. It does two jobs. It publishes the
## fuel numbers to the planner through [method get_sim_properties], so goals and action
## costs can read [code]current_fuel[/code] from the simulated world. And it hands the
## planner the two actions that act on a campfire, [AddFuelAction] and
## [CookPotatoAction], through [method get_provided_actions]. Because the fire provides
## its own actions, an agent's config never mentions it: drop a campfire into a scene
## and every agent can use it. The same holds for [WoodPileObject] and [PotatoObject].[br]
##[br]
## The node is only data and a source of actions. The fuel burning down and the label
## and flame visuals are [CampfireRuntime]'s job on the prefab root, kept apart so this
## data node could be reused with a different presentation.[br]
##[br]
## The planner reads a [i]copy[/i] of [member current_fuel] taken when planning starts,
## so a simulated Add Fuel never changes the real fire. Only [method AddFuelAction.perform_action]
## does, once the agent actually delivers the wood. Because the fire keeps burning during
## a plan, the number a plan was built from drifts; replanning absorbs that.

## Interactable metadata used by actions that target this campfire.
@export var interactable_attribs: GdPAIInteractable
## Location metadata used by navigation requirements.
@export var location_data: GdPAILocationData
## Fuel added to the campfire when one wood item is consumed.
@export var fuel_per_wood: float = 30.0
## Minimum fuel required before food can be cooked.
@export var min_fuel_to_cook: float = 20.0

## Starting fuel level in percent. Both campfire scenes begin half-full so
## fire-maintenance scenarios trigger without a long burn-down wait.
@export var current_fuel: float = 50.0
## Percent of fuel the fire loses per second; applied by [CampfireRuntime]. Not exported:
## it is a fixed pacing choice that makes a full fire last about half a minute.
var fuel_decay_rate: float = 3.0


# Override
func get_group_labels() -> Array[String]:
	# The "CampfireObject" label is what goals and world-object preconditions query by.
	return ["CampfireObject", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	# Both actions share the same location_data: it is the campfire's identity for the
	# at_target requirement, so Go To is planned toward this fire specifically.
	return [
		AddFuelAction.new(self, location_data, interactable_attribs, fuel_per_wood),
		CookPotatoAction.new(self, location_data, interactable_attribs, min_fuel_to_cook),
	]


# Override
func get_sim_properties() -> Dictionary:
	# current_fuel is the one the planner needs: MaintainFireGoal targets it and the
	# actions' costs read it. The other two are exposed for completeness; the actions
	# currently receive them through their constructors instead.
	return {
		"fuel_per_wood": fuel_per_wood,
		"current_fuel": current_fuel,
		"min_fuel_to_cook": min_fuel_to_cook,
	}
