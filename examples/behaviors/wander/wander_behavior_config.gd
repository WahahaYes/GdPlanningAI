class_name WanderBehaviorConfig
extends GdPAIBehaviorConfig
## Behavior configuration that gives an agent a goal to wander around the environment.
##[br]
##[br]
## Apply to an agent to add [WanderGoal] and [WanderAction].
##[br]
##[br]
## Wander is the idle behavior that sits beside [HungerBehaviorConfig] in
## [code]hunger_wander_agent_config.tres[/code]. It registers no [GoToAction] of its own
## because [WanderAction] already extends it; the hunger config supplies the plain one.
## It exists so an agent with nothing urgent to do keeps moving: the goal's reward is a
## small constant, so any real need (hunger above 5) takes priority over it.

## How far from its current position the agent will wander each action.
@export var wander_distance: float = 256.0


# Override
func _populate(
	goals: Array[Goal],
	actions: Array[Action],
	_updaters: Array[PropertyUpdater],
) -> void:
	goals.append(WanderGoal.new())
	actions.append(WanderAction.new(wander_distance))
