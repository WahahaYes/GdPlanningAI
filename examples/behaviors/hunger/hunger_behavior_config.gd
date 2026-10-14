class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Hunger behavior: a [HungerGoal], an [EatHeldFoodAction], a [GoToAction] and a
## [HungerPropertyUpdater], packaged so a scene adds or removes the whole concern at once.[br]
## An agent config can list several behavior configs; the agent merges their goals, actions
## and updaters, and the planner picks among all goals.

## Hunger gained per second (0 is full, 100 is starving).
@export var hunger_decay: float = 1.25
## Starting hunger value on agent initialization.
@export var initial_hunger: float = 0.0
## Maps held food item ids to the hunger they restore when consumed.
@export var hunger_restored_by_item: Dictionary = {
	"banana": 20.0,
	"cooked_potato": 50.0,
}
## Seconds to eat.
@export var eat_duration: float = 1.5
## Hunger the simulation restores before a held item is bound.
@export var optimistic_unbound_restore: float = 20.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	# Provides at_target, which every interaction action requires.
	actions.append(GoToAction.new())
	(
		actions
		. append(
			(
				EatHeldFoodAction
				. new(
					hunger_restored_by_item,
					eat_duration,
					optimistic_unbound_restore,
				)
			)
		)
	)
	updaters.append(HungerPropertyUpdater.new(hunger_decay, initial_hunger))
