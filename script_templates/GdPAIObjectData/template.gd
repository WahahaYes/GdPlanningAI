# meta-description: GdPAIObjectData template.
# meta-default: true
extends GdPAIObjectData


# Override
func get_group_labels() -> Array[String]:
	return ["REPLACE ME", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	# Return properties that should be visible to the planner during simulation.
	return {}


# Override
func get_provided_actions() -> Array[Action]:
	# Return actions that become available because this object exists in the world.
	return []
