class_name HungerPropertyUpdater
extends PropertyUpdater
## Creates the agent's [code]hunger[/code] property and raises it every frame, capped at 100.
## Eating is the only thing that lowers it.

## How much hunger rises per second.
var hunger_decay: float = 5.0
## The starting hunger value.
var initial_hunger: float = 0.0


func _init(
	p_hunger_decay: float = 5.0,
	p_initial_hunger: float = 0.0,
) -> void:
	hunger_decay = p_hunger_decay
	initial_hunger = p_initial_hunger


# Override
func initialize(agent: GdPAIAgent) -> void:
	agent.blackboard.set_property("hunger", initial_hunger)


# Override
func update_properties(agent: GdPAIAgent, delta: float) -> void:
	var current_hunger: float = agent.blackboard.get_property("hunger")
	var new_hunger: float = min(100.0, current_hunger + hunger_decay * delta)
	agent.blackboard.set_property("hunger", new_hunger)
