class_name GdPAIUTILS
extends Object
## Static scene-tree helpers used throughout the addon. Safe to use from your own scripts, but
## only from the main thread and never from planning-time hooks.


## Returns the first node that is [param node] itself or a descendant of it and is an instance of
## [param _class] (depth-first, checking each node's children before recursing), or
## [code]null[/code] if there is none. [param _class] is a class or script, as accepted by
## [code]is_instance_of[/code].
static func get_child_of_type(
	node: Node,
	_class: Variant,
) -> Variant:
	if is_instance_of(node, _class):
		return node
	for child in node.get_children():
		if is_instance_of(child, _class):
			return child
		var recursive_value = get_child_of_type(child, _class)
		if recursive_value != null:
			return recursive_value
	return null


## Returns all nodes in the scene tree belonging to [param group] that are [param node] itself or
## a descendant of it. [param node] must be inside the tree.
static func get_children_in_group(node: Node, group: String) -> Array:
	var result: Array = []
	for child in node.get_tree().get_nodes_in_group(group):
		if child == node or node.is_ancestor_of(child):
			result.append(child)
	return result
