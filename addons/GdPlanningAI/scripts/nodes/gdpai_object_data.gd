class_name GdPAIObjectData
extends Node
## Makes a scene object visible to the planner. Extend it per kind of world object (a campfire,
## a food source).[br]
## On each plan request the object offers [method get_provided_actions] and is snapshotted from
## [method get_sim_properties] into a [SimObjectProxy] that planning hooks read and write instead
## of the live node.

## Top-level node of the object, for actions that need the real node during execution.
@export var entity: Node


## Adds the node to every group returned by [method get_group_labels].
func _init() -> void:
	for label in get_group_labels():
		add_to_group(label)


## Returns the groups this node joins in [code]_init[/code]; preconditions find objects by group.
## Keep [code]"GdPAIObjectData"[/code] and the base class's labels in overrides.
func get_group_labels() -> Array[String]:
	return ["GdPAIObjectData"]


## Returns fresh [Action]s this object offers to agents. Called on every plan request.
func get_provided_actions() -> Array[Action]:
	return []


## Returns the properties the planner may read or change, copied into the snapshot. A string
## [code]provides[/code] property also provides that fact, with this object's id as its argument.
func get_sim_properties() -> Dictionary:
	return {}
