class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent hunger and a goal to eat food.
##[br]
##[br]
## Apply to an agent to add [HungerGoal] and [HungerPropertyUpdater].
## Configurable via [member hunger_decay] and [member initial_hunger].


## How much hunger drops per second.
@export var hunger_decay: float = 2.5
## Starting hunger value on agent initialization.
@export var initial_hunger: float = 100.0


# Override
func _populate(
		goals: Array[Goal],
		_actions: Array[Action],
		updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
