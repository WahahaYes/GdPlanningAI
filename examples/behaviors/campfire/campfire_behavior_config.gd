class_name CampfireBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent campfire-maintenance behavior.
##[br]
##[br]
## Apply alongside [HungerBehaviorConfig] so the same agent can balance personal
## food needs with shared fire maintenance.


## Fixed reward value assigned to the fire-maintenance goal.
@export var fire_goal_reward: float = 40.0
## Fire fuel threshold considered "maintained" for planning purposes.
@export var desired_fuel_level: float = 60.0
## How long dropping a held item should take in seconds.
@export var drop_duration: float = 0.2


# Override
func _populate(
		goals: Array[Goal],
		actions: Array[Action],
		_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(MaintainFireGoal.new(fire_goal_reward, desired_fuel_level))
	actions.append(DropItemAction.new(drop_duration))
