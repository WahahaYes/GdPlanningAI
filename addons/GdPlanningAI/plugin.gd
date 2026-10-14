@tool
extends EditorPlugin
## Editor plugin for GdPlanningAI. Enabling it registers the [code]GdPAIAutoload[/code] singleton
## (see [GdPAIAutoload]); disabling it removes the singleton again. It has no other behavior.


# Override
## Names the plugin.
func _init() -> void:
	name = "GdPlanningAI"


# Override
## Registers the [code]GdPAIAutoload[/code] autoload.
func _enter_tree() -> void:
	add_autoload_singleton("GdPAIAutoload", "gdpai_autoload.gd")


# Override
## Unregisters the [code]GdPAIAutoload[/code] autoload.
func _exit_tree() -> void:
	remove_autoload_singleton("GdPAIAutoload")
