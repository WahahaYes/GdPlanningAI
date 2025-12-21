extends Node
## Sample initialization for an GdPAI agent which gives actions and goals corresponding to the agent
## wanting to eat nearby food.

## Reference to the GdPAI agent.
@export var gdpai_agent: GdPAIAgent
## How much hunger drops per second.
@export var hunger_decay: float = 5


func _ready() -> void:
	# Set up the world node, agent goals, and agent available actions.
	gdpai_agent.world_node = GdPAIUTILS.get_child_of_type(get_tree().root, GdPAIWorldNode)
	gdpai_agent.goals.append(SampleHungerGoal.new())
	gdpai_agent.goals.append(SampleWanderGoal.new())
	# Here the SampleWanderAction is being created with wander_distance of 256px.
	gdpai_agent.self_actions.append(SampleWanderAction.new(256))


func _process(delta: float) -> void:
	# Update the agent's hunger status continuously.
	var hunger: float = gdpai_agent.blackboard.get_property("hunger")
	hunger = max(0, hunger - hunger_decay * delta)
	gdpai_agent.blackboard.set_property("hunger", hunger)
