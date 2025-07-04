@tool


## Searches a node's tree to find the first instance of _class.
static func get_child_of_type(node: Node, _class: Variant) -> Variant:
	if is_instance_of(node, _class):
		return node
	for child in node.get_children():
		if is_instance_of(child, _class):
			return child
		var recursive_value = get_child_of_type(child, _class)
		if recursive_value != null:
			return recursive_value
	return null


## Searches a node's tree to find all children of type _class.
static func get_children_of_type(node: Node, _class: Variant) -> Array[Variant]:
	var children: Array = []
	return _get_children_of_type(children, node, _class)


static func _get_children_of_type(children: Array, node: Node, _class: Variant):
	if is_instance_of(node, _class):
		children.append(node)
	for child in node.get_children():
		children = _get_children_of_type(children, child, _class)
	return children
