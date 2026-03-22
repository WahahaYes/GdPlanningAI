# meta-description: GdPAIObjectData template.
# meta-default: true
extends GdPAIObjectData


# Override
func _init() -> void:
	# In case extending _init(), make sure to call super() so that the group is assigned.
	super()


# Override
func get_group_labels() -> Array[String]:
	# Make sure to add a group label for this class of data.
	return ["REPLACE ME", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	# Overwrite the get_provided_actions function to serve any actions that become possible because
	# this object exists out in the world.
	return []


