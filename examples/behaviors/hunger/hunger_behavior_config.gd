class_name HungerBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent hunger and a goal to eat food.
##[br]
##[br]
## This is the first thing to read when learning how a behavior is packaged. One
## [GdPAIBehaviorConfig] bundles everything a single concern needs: a [HungerGoal] that
## says what the agent wants, an [EatHeldFoodAction] that can satisfy it, a [GoToAction]
## for walking, and a [HungerPropertyUpdater] that makes the [code]hunger[/code]
## blackboard property exist and grow. Listing the pieces in one resource means a scene
## designer adds or removes the whole concern in the agent config instead of wiring four
## scripts by hand.[br]
##[br]
## An agent config lists several behavior configs and the agent merges them: it calls
## [method GdPAIBehaviorConfig.apply_to_agent] on each in order, appending every goal,
## self action and updater to its own lists. The planner then chooses among all goals.
## [code]campfire_agent_config.tres[/code] pairs this config with
## [CampfireBehaviorConfig]; [code]hunger_wander_agent_config.tres[/code] pairs it with
## [WanderBehaviorConfig]. Each config registers its own [GoToAction] so it still works
## when used alone.[br]
##[br]
## The exported values are tuning knobs that reach the pieces through their
## constructors, so the goal and action scripts stay free of scene specifics.
## See [member hunger_decay] and [member initial_hunger] for the tunables.

## How much hunger rises per second. Hunger counts up (0 is full, 100 is starving), so
## the property updater adds this each second rather than subtracting it.
@export var hunger_decay: float = 1.25
## Starting hunger value on agent initialization.
@export var initial_hunger: float = 0.0
## Maps held food item ids to the hunger they restore when consumed.
@export var hunger_restored_by_item: Dictionary = {
	"banana": 20.0,
	"cooked_potato": 50.0,
}
## How long the self-eating action should take in seconds.
@export var eat_duration: float = 1.5
## Planning-only hunger restore assumed before a held item provider is bound.
@export var optimistic_unbound_restore: float = 20.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	updaters: Array[PropertyUpdater],
) -> void:
	goals.append(HungerGoal.new())
	# Every interaction action requires an at_target fact and only GoToAction provides
	# it, so any agent that interacts with objects needs this in its self actions.
	actions.append(GoToAction.new())
	# Eating acts on the agent itself (the held item), so it is a self action registered
	# here rather than an action provided by a world object.
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
