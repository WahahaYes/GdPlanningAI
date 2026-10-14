# meta-description: GdPAIObjectData template.
# meta-default: true
extends GdPAIObjectData


# Override
func get_group_labels() -> Array[String]:
	# Groups preconditions use to find this object. Keep "GdPAIObjectData".
	return ["REPLACE ME", "GdPAIObjectData"]


# Override
func get_sim_properties() -> Dictionary:
	# Properties the planner may read or change, copied into a snapshot at each plan request.
	return {}


# Override
func get_provided_actions() -> Array[Action]:
	# Fresh actions this object offers agents, usually capturing self.
	return []
