extends Label
## Displays the current goal and active action of a [GdPAIAgent] as a heads-up label.
##[br]
##[br]
## Drop this script onto a [Label] node and assign [member gdpai_agent].
## The label updates every frame with the agent's current goal title and
## the title of the action currently being executed.

## The agent to display debug info for.
@export var gdpai_agent: GdPAIAgent


func _process(_delta: float) -> void:
	var goal_text: String = ""
	var current_goal: Goal = gdpai_agent.get_current_goal()
	if current_goal != null:
		goal_text = current_goal.get_title()

	var action_text: String = ""
	var chain: Array[Action] = gdpai_agent.get_current_plan()
	var step: int = gdpai_agent.get_current_plan_step()
	if not chain.is_empty() and step >= 0 and step < chain.size():
		action_text = chain[step].get_title()

	text = "Goal: %s\nAction: %s" % [goal_text, action_text]
