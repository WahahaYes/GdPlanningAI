extends Label

## Reference to the GdPAI agent.
@export var gdpai_agent: GdPAIAgent


func _process(_delta: float) -> void:
	var hunger: float = gdpai_agent.blackboard.get_property("hunger")
	var goal_text: String
	if gdpai_agent._current_goal != null:
		goal_text = gdpai_agent._current_goal.get_title()

	var action_text: String
	if gdpai_agent._current_plan != null and gdpai_agent._current_plan.get_plan().size() > 0:
		var step: int = min(
			gdpai_agent._current_plan_step,
			gdpai_agent._current_plan.get_plan().size() - 1,
		)
		var action: Action = gdpai_agent._current_plan.get_plan()[step]
		action_text = action.get_title()

	self.text = (
		"Hunger: %.f\nGoal: %s\nCurrent Action:\n     %s" % [hunger, goal_text, action_text]
	)
