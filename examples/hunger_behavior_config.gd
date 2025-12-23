class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration for agents that experience hunger.


## Override to automatically set up hunger-related configuration.
func _init():
	goals.append(SampleHungerGoal.new())
	goals.append(SampleWanderGoal.new())
	self_actions.append(SampleWanderAction.new(256))
	property_updaters.append(HungerPropertyUpdater.new())
	initial_properties = { "hunger": 100.0 }
