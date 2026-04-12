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
@export var hunger_restored_per_meal: float = 50.0
@export var eat_duration: float = 1.5
@export var allowed_food_items: Array[String] = ["cooked_potato"]


# Override
func _populate(
		goals: Array[Goal],
		actions: Array[Action],
		updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	actions.append(EatHeldFoodAction.new(
		allowed_food_items,
		hunger_restored_per_meal,
		eat_duration,
	))
	updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
