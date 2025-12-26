class_name GdPAIBlackboard
extends RefCounted
## The GdPAIBlackboard uses an underlying dictionary to store key information about an agent's
## state or the state of the world.  Duplicated copies of the blackboard are passed through a
## chain of events when agents are planning.
##[br]
##[br]
## Has a protected property GDPAI_OBJECTS containing a list of the GdPAI objects associated with the
## agent / world.  This is ensured to exist for any given blackboard.

## Special property in blackboards to cache object data.
const GDPAI_OBJECTS = "GDPAI_OBJECTS"

## Flag to indicate that this blackboard is a copy.  If so, when it is deleted, all
## GdPAIObjectData inside it (which are also copies) are deleted to prevent memory leaks.
var is_a_copy: bool
## Underlying blackboard data structure.
var _blackboard: Dictionary = { GDPAI_OBJECTS: [] }


# Override
func _notification(what: int) -> void:
	if not is_a_copy:
		return
	match what:
		NOTIFICATION_PREDELETE:
			# When deleting a copied blackboard (in simulation), this frees all its duplicated
			# GdPAIObjectData nodes.
			for aod: GdPAIObjectData in _blackboard[GDPAI_OBJECTS]:
				if is_instance_valid(aod):
					aod.queue_free()


# Override
func _to_string() -> String:
	# For better readability when debugging, pass the underlying dict through _to_string().
	return str(_blackboard)


## Check if a property exists in the blackboard.
func has_property(prop: String) -> bool:
	return prop in _blackboard


## Return the value of a requested property, with an optional default value.
func get_property(
		prop: String,
		default_value: Variant = null,
) -> Variant:
	if prop in _blackboard:
		return _blackboard[prop]
	return default_value


## Iterate over the list of GdPAI objects in the blackboard and return any in the requested group.
func get_objects_in_group(group: String) -> Array[GdPAIObjectData]:
	var objects: Array[GdPAIObjectData] = []
	for potential_obj: GdPAIObjectData in _blackboard[GDPAI_OBJECTS]:
		if potential_obj.is_in_group(group):
			objects.append(potential_obj)
	return objects


## Return the first GdPAIObjectData in the requested group.
func get_first_object_in_group(group: String) -> GdPAIObjectData:
	var objects: Array[GdPAIObjectData] = get_objects_in_group(group)
	if objects.size() == 0:
		return null
	return objects[0]


## Return a GdPAIObjectData instance contained in this blackboard by its referenced uid.
func get_object_by_uid(uid: String) -> GdPAIObjectData:
	for potential_obj: GdPAIObjectData in _blackboard[GDPAI_OBJECTS]:
		if potential_obj.uid == uid:
			return potential_obj
	return null


## Set the value of a specified property.
func set_property(
		prop: String,
		value: Variant,
) -> void:
	_blackboard[prop] = value


## Erase a property from the blackboard entirely.
func erase_property(prop: String) -> void:
	_blackboard.erase(prop)


## Returns the underlying dictionary of the blackboard.
func get_dict() -> Dictionary:
	return _blackboard


## Overwrites the underlying dictionary of the blackboard.
func set_dict(new_dict: Dictionary) -> void:
	_blackboard = new_dict


## Append all properties and their associated values from another blackboard into this one.  This
## will overwrite the values of any already existing properties.
func append_other_blackboard(other_blackboard: GdPAIBlackboard) -> void:
	for key in other_blackboard.get_dict():
		set_property(key, other_blackboard.get_property(key))


## Duplicate this object by duplicating the underlying dictionary so that changes to the copy do
## not effect the original.
func copy_for_simulation() -> GdPAIBlackboard:
	var duplicate: GdPAIBlackboard = GdPAIBlackboard.new()
	for key in _blackboard.keys():
		if key == GDPAI_OBJECTS:
			continue
		duplicate._blackboard[key] = _blackboard[key]
	var duped_objects: Array[GdPAIObjectData] = []
	# TODO: This can throw a c++ error but continues.  The next if statement protects the code,
	# 		but it might be nice to revisit this (might need a custom iterator).
	# NOTE: Before custom iterator try hooking up a signal between object on_destroy and a
	#		function that pops the element from _blackboard[GDPAI_OBJECTS] automatically.
	for aod: GdPAIObjectData in _blackboard[GDPAI_OBJECTS]:
		# aod.copy_for_simulation() preserves uids and properties but detaches from the scene graph.
		if aod != null and is_instance_valid(aod):
			duped_objects.append(aod.copy_for_simulation())
	duplicate._blackboard[GDPAI_OBJECTS] = duped_objects
	return duplicate
