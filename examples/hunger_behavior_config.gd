class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration for agents that experience hunger.

## How much hunger drops per second.
@export var hunger_decay: float = 5.0


## Override to automatically set up hunger-related configuration.
func _init():
	goals.append(SampleHungerGoal.new())
	goals.append(SampleWanderGoal.new())
	self_actions.append(SampleWanderAction.new(256))
	property_updaters.append(HungerPropertyUpdater.new(hunger_decay))
	initial_properties = { "hunger": 100.0 }
