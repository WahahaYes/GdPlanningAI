@tool
class_name GdPlanningAIDebugger
extends EditorDebuggerPlugin

var debugger_tab: GdPlanningAIDebuggerTab = GdPlanningAIDebuggerTab.new()
var session: EditorDebuggerSession


func _has_capture(prefix: String) -> bool:
	return prefix == "gdplanningai"


func _capture(message: String, data: Array, session_id: int) -> bool:
	if message == "gdplanningai:register_agent":
		debugger_tab.register_agent(data[0], data[1])
		return true
	if message == "gdplanningai:unregister_agent":
		debugger_tab.unregister_agent(data[0])
		return true
	if message == "gdplanningai:update_plan":
		debugger_tab.update_plan(data[0], data[1])
		return true
	return false


func _setup_session(session_id: int) -> void:
	session = get_session(session_id)
	debugger_tab.name = "📊 GdPlanningAI"
	session.add_session_tab(debugger_tab)
