class_name HungerPropertyUpdater
extends PropertyUpdater
## Property updater that decreases hunger over time.

## How much hunger drops per second.
var hunger_decay: float = 5.0


func _init(p_hunger_decay: float = 5.0) -> void:
	hunger_decay = p_hunger_decay


## Override to update hunger property over time.
func update_properties(agent: GdPAIAgent, delta: float) -> void:
	var current_hunger: float = agent.blackboard.get_property("hunger")
	var new_hunger: float = max(0.0, current_hunger - hunger_decay * delta)
	agent.blackboard.set_property("hunger", new_hunger)


## Override to set initial hunger value.
func initialize(agent: GdPAIAgent) -> void:
	agent.blackboard.set_property("hunger", 100.0)
