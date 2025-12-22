@tool
class_name GdPAIDebugger
extends EditorDebuggerPlugin
## Debugger plugin for GdPlanningAI system.

## Debugger tab reference created at runtime.
var debugger_tab: GdPAIDebuggerTab = GdPAIDebuggerTab.new()
## Current debugger session.
var session: EditorDebuggerSession


# Override
func _has_capture(prefix: String) -> bool:
	return prefix == "gdplanningai"


# Override
func _capture(message: String, data: Array, _session_id: int) -> bool:
	if message == "gdplanningai:register_agent":
		# (agent_id, agent_name)
		debugger_tab.register_agent(data[0], data[1])
		return true
	if message == "gdplanningai:unregister_agent":
		# (agent_id)
		debugger_tab.unregister_agent(data[0])
		return true
	if message == "gdplanningai:update_agent_info":
		# (agent_id, agent_info { plan_tree, ... })
		debugger_tab.update_agent_info(data[0], data[1])
		return true
	if message == "gdplanningai:clear_state":
		debugger_tab.clear_state()
		return true
	return false


# Override
func _setup_session(session_id: int) -> void:
	session = get_session(session_id)
	debugger_tab.name = "🧠GdPlanningAI"
	session.add_session_tab(debugger_tab)
