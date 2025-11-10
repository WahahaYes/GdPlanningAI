@tool
extends EditorPlugin

var debugger: GdPlanningAIDebugger


func _enter_tree() -> void:
	debugger = GdPlanningAIDebugger.new()
	add_debugger_plugin(debugger)


func _exit_tree() -> void:
	remove_debugger_plugin(debugger)
