class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration for agents that experience hunger.

## How much hunger drops per second.
@export var hunger_decay: float = 5.0
## The starting hunger value.
@export var initial_hunger: float = 100.0


# Override
func _self_init():
	super()
	goals.append(SampleHungerGoal.new())
	property_updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
