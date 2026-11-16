# SPDX-License-Identifier: Apache-2.0
# Copyright 2025 Ethan Wilson
# https://github.com/WahahaYes/GdPlanningAI

class_name Goal
extends RefCounted
## Something an agent wants to achieve. Extend it to write a new goal.[br]
## [method get_reward] says how much the agent wants it now; [method get_desired_state] says
## when it is achieved. Both are read once per plan request, before the search, and may read live
## scene state. Register goals through a [GdPAIBehaviorConfig].


# Override
## Returns how much the agent wants this goal now. Goals are tried from highest reward down.
func get_reward(_agent: GdPAIAgent) -> float:
	return 0


# Override
## Returns the conditions that must all hold for this goal to be achieved. Prefer built-in
## preconditions: the planner can search for actions that satisfy them.
func get_desired_state(_agent: GdPAIAgent) -> Array[Precondition]:
	return []


## Returns a short name for logs; also sent to the engine.
func get_title() -> String:
	return "Goal"


## Returns a longer description for debug output.
func get_description() -> String:
	return "Base class for Goal."
