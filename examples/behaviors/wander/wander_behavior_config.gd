class_name WanderBehaviorConfig
extends GdPAIBehaviorConfig
## Idle behavior: a [WanderGoal] with a small reward, so any real need takes priority. Adds no
## plain [GoToAction]; pair it with a config that does.

## Distance of each wander step.
@export var wander_distance: float = 256.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(WanderGoal.new())
	actions.append(WanderAction.new(wander_distance))
