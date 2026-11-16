# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

@tool
extends EditorPlugin
## Editor plugin for GdPlanningAI. Enabling it registers the [code]GdPAIAutoload[/code] singleton
## (see [GdPAIAutoload]); disabling it removes the singleton again. It has no other behavior.


# Override
## Names the plugin.
func _init() -> void:
	name = "GdPlanningAI"


# Override
## Registers the [code]GdPAIAutoload[/code] autoload when the plugin is enabled.
func _enable_plugin() -> void:
	add_autoload_singleton("GdPAIAutoload", "gdpai_autoload.gd")


# Override
## Unregisters the [code]GdPAIAutoload[/code] autoload when the plugin is disabled.
func _disable_plugin() -> void:
	remove_autoload_singleton("GdPAIAutoload")
