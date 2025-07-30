# meta-description: GdPAIObjectData template.
# meta-default: true

extends GdPAIObjectData


# Override
func _init() -> void:
	# In case extending _init(), make sure to call super() so that the group is assigned.
	super()


# Override
func get_group_labels():
	# Make sure to add a group label for this class of data.
	return ["REPLACE ME", "GdPAIObjectData"]


# Override
func get_provided_actions() -> Array[Action]:
	# Overwrite the get_provided_actions function to serve any actions that become possible because
	# this object exists out in the world.
	return []


# Override
func copy_for_simulation() -> GdPAIObjectData:
	# Make sure to replace <GdPAIObjectData> with the subclass name, and to duplicate any new properties.
	var new_data: GdPAIObjectData = GdPAIObjectData.new()
	assign_uid_and_entity(new_data)
	return new_data
