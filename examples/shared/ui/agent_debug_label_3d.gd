extends Label3D
## Displays the current goal, active action, and specified
## properties of a [GdPAIAgent] as a heads-up label for 3D scenes.
##[br]
##[br]
## Drop this script onto a [Label3D] node and assign [member gdpai_agent].
## The label updates every frame with the agent's current goal title,
## the title of the action currently being executed, and any properties
## specified in [member debug_properties].
##[br]
##[br]
## Float values are automatically truncated to 1 decimal place
## for cleaner display.

## The agent to display debug info for.
@export var gdpai_agent: GdPAIAgent
## Array of property names to display from the agent's blackboard.
## Example: ["hunger", "held_item"]
@export var debug_properties: Array[String] = []


## Formats a value for display, truncating floats to 1 decimal place.
func _format_value(value: Variant) -> String:
	if value is float:
		return "%.1f" % value
	return str(value)


func _process(_delta: float) -> void:
	if not is_instance_valid(gdpai_agent):
		return

	var goal_text: String = ""
	var current_goal: Goal = gdpai_agent.get_current_goal()
	if current_goal != null:
		goal_text = current_goal.get_title()

	var action_text: String = ""
	var chain: Array[Action] = gdpai_agent.get_current_plan()
	var step: int = gdpai_agent.get_current_plan_step()
	if not chain.is_empty() and step >= 0 and step < chain.size():
		action_text = chain[step].get_title()

	var props_text: String = ""
	if not debug_properties.is_empty():
		var bb: GdPAIBlackboard = gdpai_agent.blackboard
		var prop_strings: Array[String] = []
		for prop_name in debug_properties:
			var value = bb.get_property(prop_name)
			prop_strings.append("%s: %s" % [prop_name, _format_value(value)])
		props_text = "\n" + "\n".join(prop_strings)

	text = "Goal: %s\nAction: %s%s" % [goal_text, action_text, props_text]
