class_name HungerPropertyUpdater
extends PropertyUpdater
## Grows the agent's [code]hunger[/code] blackboard property over time and initializes
## it to a starting value when the agent is set up.
##[br]
##[br]
## A property updater is how an example gives an agent a need that changes on its own.
## The updater owns the property from start to finish: [method initialize] creates it
## (so goals and actions can read it from the first plan) and [method update_properties]
## advances it every frame. Keeping that out of goals and actions leaves them pure
## readers of the blackboard. Hunger rises toward 100 and is capped there; eating
## ([EatHeldFoodAction]) is the only thing that lowers it.[br]
##[br]
## The class default of 5 per second is a placeholder; [HungerBehaviorConfig] passes
## its own [member HungerBehaviorConfig.hunger_decay] (1.25 per second).

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
