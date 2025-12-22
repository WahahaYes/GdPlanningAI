@tool
extends EditorPlugin
## Editor plugin for GdPlanningAI.

## Debugger instance.
var debugger: GdPAIDebugger


# Override
func _init() -> void:
	name = "GdPlanningAI"


# Override
func _enter_tree() -> void:
	debugger = GdPAIDebugger.new()
	add_debugger_plugin(debugger)
	add_autoload_singleton("GdPAIAutoload", "gdpai_autoload.gd")


# Override
func _exit_tree() -> void:
	remove_debugger_plugin(debugger)
	remove_autoload_singleton("GdPAIAutoload")
