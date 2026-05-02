class_name GdPAIUTILS
extends Object
## Static utility classes for the GdPlanningAI addon.


## Searches a node's tree to find the first instance of _class.
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


## Returns all nodes belonging to [param group] that are [param node] itself or a descendant of it.


static func get_children_in_group(node: Node, group: String) -> Array:
	var result: Array = []
	for child in node.get_tree().get_nodes_in_group(group):
		if child == node or node.is_ancestor_of(child):
			result.append(child)
	return result
