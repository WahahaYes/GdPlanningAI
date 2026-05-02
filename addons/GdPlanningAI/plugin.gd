@tool
extends EditorPlugin
## Editor plugin for GdPlanningAI.


# Override
func _init() -> void:
	name = "GdPlanningAI"


# Override


func _enter_tree() -> void:
	add_autoload_singleton("GdPAIAutoload", "gdpai_autoload.gd")


# Override


func _exit_tree() -> void:
	remove_autoload_singleton("GdPAIAutoload")
