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
@export var initial_hunger: float = 0.0
## Maps held food item ids to the hunger they restore when consumed.
@export var hunger_restored_by_item: Dictionary = {
	"banana": 20.0,
	"cooked_potato": 50.0,
}
## How long the self-eating action should take in seconds.
@export var eat_duration: float = 1.5


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	actions.append(GoToAction.new())
	(
		actions
		. append(
			(
				EatHeldFoodAction
				. new(
					hunger_restored_by_item,
					eat_duration,
				)
			)
		)
	)
	updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
