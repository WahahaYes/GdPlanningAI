class_name WanderBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration for agents that wander around the environment.

## Distance the agent will wander in each action.
@export var wander_distance: float = 20.0


# Override
func _populate(
		goals: Array[Goal],
		actions: Array[Action],
		_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(SampleWanderGoal.new())
	actions.append(SampleWanderAction.new(wander_distance))
