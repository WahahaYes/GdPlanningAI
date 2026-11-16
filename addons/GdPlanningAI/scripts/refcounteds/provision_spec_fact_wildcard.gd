# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

class_name ProvisionSpecFactWildcard
extends ProvisionSpec
## Provision of a fact that satisfies any requirement with the same fact name, whatever its
## arguments, so a single action can serve many different requirements (for example [GoToAction]
## navigating to any location). Create it with [method ProvisionSpec.fact_wildcard].
## @experimental: Internal to GdPlanningAI; may change in any release.

## Name of the provided fact.
var fact_name: String


## Creates the provision for fact [param name].
func _init(name: String) -> void:
	fact_name = name


## Serialises this provision for the Rust bridge.
func to_bridge_dict() -> Dictionary:
	return {
		"kind": "fact_wildcard",
		"fact_name": fact_name,
	}
