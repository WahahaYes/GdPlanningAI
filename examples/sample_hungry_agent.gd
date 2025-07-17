## Sample initialization for an GdPAI agent which gives actions and goals corresponding to the agent
## wanting to eat nearby food.
extends Node

## A reference to utils for the GdPAI addon.
const GdPAIUTILS: Resource = preload("res://addons/GdPlanningAI/utils.gd")

## Reference to the GdPAI agent.
@export var GdPAI_agent: GdPAIAgent
## How much hunger drops per second.
@export var hunger_decay: float = 5

## Reference to a textual display which shows some attributes of the agent.
@export var display_text: Label


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

	# Hacky way to display the goal and action being considered at this moment.
	var goal_text: String
	if GdPAI_agent._current_goal != null:
		goal_text = GdPAI_agent._current_goal.get_script().resource_path.split("/")[-1]

	var action_text: String
	if GdPAI_agent._current_plan != null and GdPAI_agent._current_plan.get_plan().size() > 0:
		var step: int = min(
			GdPAI_agent._current_plan_step, GdPAI_agent._current_plan.get_plan().size() - 1
		)
		var action: Action = GdPAI_agent._current_plan.get_plan()[step]
		action_text = action.get_script().resource_path.split("/")[-1]

	display_text.text = (
		"Hunger: %.f\nGoal: %s\n Current\nAction: %s" % [hunger, goal_text, action_text]
	)
