class_name CampfireBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent campfire-maintenance behavior.
##[br]
##[br]
## Apply alongside [HungerBehaviorConfig] so the same agent can balance personal
## food needs with shared fire maintenance. This is the example of two configs
## merging on one agent: the agent ends up with both goals ([HungerGoal] and
## [MaintainFireGoal]) and the planner picks whichever currently has the higher reward.
## [member fire_goal_reward] is set to 40 so tending the fire beats hunger while the
## agent is only mildly hungry (hunger reward is the hunger value, so it overtakes the
## fire at hunger 40) and the fire never starves because the agent is busy eating.[br]
##[br]
## The config registers only the pieces the fire goal needs. The actions that actually
## touch the fire ([AddFuelAction], [CookPotatoAction]) are not here: the campfire and
## wood pile objects provide them to the planner (see [CampfireObject] and
## [WoodPileObject]), so adding a campfire to a scene needs no change to the agent.

## Fixed reward value assigned to the fire-maintenance goal.
@export var fire_goal_reward: float = 40.0
## Fire fuel threshold considered "maintained" for planning purposes.
@export var desired_fuel_level: float = 60.0
## Group of the campfire(s) this agent tends. Empty tends any campfire.
@export var campfire_group: String = ""
## How long dropping a held item should take in seconds.
@export var drop_duration: float = 0.2


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(MaintainFireGoal.new(fire_goal_reward, desired_fuel_level, campfire_group))
	# Duplicates the GoToAction from HungerBehaviorConfig on purpose, so this config
	# also works for an agent that does not use the hunger behavior.
	actions.append(GoToAction.new())
	# With one held-item slot, an agent carrying the wrong thing (a potato when it
	# should fetch wood) is stuck; dropping frees the slot so a new plan can start.
	actions.append(DropItemAction.new(drop_duration))
