class_name WanderBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent a goal to wander around the environment.
##[br]
##[br]
## Apply to an agent to add [WanderGoal] and [WanderAction].

## How far from its current position the agent will wander each action.
@export var wander_distance: float = 256.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(WanderGoal.new())
	actions.append(WanderAction.new(wander_distance))
