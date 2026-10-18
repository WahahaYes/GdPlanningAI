class_name CampfireBehaviorConfig
extends GdPAIBehaviorConfig
## Campfire-maintenance behavior; combine with [HungerBehaviorConfig] on the same agent, which
## supplies the [GoToAction] every fire action needs.[br]
## The planner pursues whichever goal rewards more. A reward of 40 means the fire wins until
## hunger passes 40. The fire actions themselves come from the campfire and wood pile objects,
## so they are not registered here.

## Reward of the fire goal, compared against hunger.
@export var fire_goal_reward: float = 40.0
## Fuel level that counts as maintained.
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
	# Frees the single held-item slot when the agent carries the wrong thing.
	actions.append(DropItemAction.new(drop_duration))
