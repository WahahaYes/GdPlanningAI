class_name PropertyUpdater
extends RefCounted
## Changes an agent's blackboard over time, outside planning (hunger rising, stamina
## regenerating). Register it from [method GdPAIBehaviorConfig._populate].


# Override
## Called every frame before the agent plans or acts.
func update_properties(
	_agent: GdPAIAgent,
	_delta: float,
) -> void:
	pass


# Override
## Sets initial values. Called once when the behavior config is applied.
func initialize(_agent: GdPAIAgent) -> void:
	pass
