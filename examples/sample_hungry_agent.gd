## Sample initialization for an GdPAI agent which gives actions and goals corresponding to the agent
## wanting to eat nearby food.
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")

## Reference to the GdPAI agent.
@export var GdPAI_agent: GdPAIAgent
## How much hunger drops per second.
@export var hunger_decay: float = 5


func _ready() -> void:
	# Set up the world node, agent goals, and agent available actions.
	GdPAI_agent.world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	GdPAI_agent.goals.append(SampleHungerGoal.new())
	GdPAI_agent.goals.append(SampleWanderGoal.new())
	# Here the SampleWanderAction is being created with wander_distance of 256px.
	GdPAI_agent.self_actions.append(SampleWanderAction.new(256))


func _process(delta: float) -> void:
	# Update the agent's hunger status continuously.
	var hunger: float = GdPAI_agent.blackboard.get_property("hunger")
	hunger = max(0, hunger - hunger_decay * delta)
	GdPAI_agent.blackboard.set_property("hunger", hunger)
