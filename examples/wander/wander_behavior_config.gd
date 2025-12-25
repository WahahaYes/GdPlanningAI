class_name WanderBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration for agents that wander around the environment.

## Distance the agent will wander in each action.
@export var wander_distance: float = 20.0


func _init():
	goals.append(SampleWanderGoal.new())
	self_actions.append(SampleWanderAction.new(wander_distance))
