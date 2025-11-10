@tool
class_name GdPlanningAIDebugger
extends EditorDebuggerPlugin

var debugger_tab: GdPlanningAIDebuggerTab = GdPlanningAIDebuggerTab.new()
var session: EditorDebuggerSession


func _has_capture(prefix: String) -> bool:
	return prefix == "gdplanningai"


func _capture(message: String, data: Array, session_id: int) -> bool:
	# We'll implement message handling later
	return false


func _setup_session(session_id: int) -> void:
	session = get_session(session_id)
	debugger_tab.name = "📊 GdPlanningAI"
	session.add_session_tab(debugger_tab)
