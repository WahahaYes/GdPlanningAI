@tool

static var mutex: Mutex = Mutex.new()


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


## Waits for a deferred call to the main thread to return information.
static func await_callv(obj: Object, method: String, args: Array = []) -> Variant:
	return await callv_deferred(obj, method, args).finished


## Makes a deferred call with the option to listen for a finished signal.
static func callv_deferred(obj: Object, method: String, args: Array = []) -> AwaitableCallDeferred:
	return AwaitableCallDeferred.new(obj, method, args)


## Internal class that structures a call_deferred() call and a returning signal to query info from
## the main thread (somewhat) safely.
class AwaitableCallDeferred:
	signal finished(result)

	func _init(obj: Object, method: String, args: Array = []) -> void:
		call_deferred("_call_and_signal", obj, method, args)

	func _call_and_signal(obj: Object, method: String, args: Array = []) -> void:
		var result = obj.callv(method, args)
		emit_signal("finished", result)
