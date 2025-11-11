@tool
class_name GdPlanningAIPlanGraphEdit
extends GraphEdit

const H_SPACING := 260.0
const V_SPACING := 150.0

var plan_tree: Dictionary:
	set(value):
		if _plan_tree == value:
			return
		_plan_tree = value
		_update_graph()
	get:
		return _plan_tree
var _plan_tree: Dictionary = { }


func _ready() -> void:
	show_arrange_button = false
	minimap_enabled = false
	grid_pattern = GraphEdit.GRID_PATTERN_DOTS


func _update_graph() -> void:
	clear_connections()

	for child in get_children():
		if child is GraphNode:
			remove_child(child)
			child.queue_free()

	if _plan_tree.is_empty():
		return

	var positions := { }
	var leaf_index := [0]
	_compute_positions(_plan_tree, 0, positions, leaf_index)
	_add_nodes(_plan_tree, positions)
	_connect_nodes(_plan_tree)
	arrange_nodes()


func _compute_positions(node: Dictionary, depth: int, positions: Dictionary, leaf_index: Array) -> float:
	var children: Array = node.get("children", [])
	var x: float

	if children.is_empty():
		x = leaf_index[0] * H_SPACING
		leaf_index[0] += 1
	else:
		var min_x := INF
		var max_x := -INF
		for child in children:
			var child_x := _compute_positions(child, depth + 1, positions, leaf_index)
			min_x = min(min_x, child_x)
			max_x = max(max_x, child_x)
		x = (min_x + max_x) / 2.0

	var node_id: String = str(node.get("id", "node_%s" % leaf_index[0]))
	positions[node_id] = Vector2(x, depth * V_SPACING)
	return x


func _add_nodes(node: Dictionary, positions: Dictionary) -> void:
	var graph_node := GdPlanningAIPlanGraphNode.new()
	graph_node.name = str(node.get("id", ""))
	add_child(graph_node)
	graph_node.position_offset = positions.get(graph_node.name, Vector2.ZERO)
	graph_node.set_node_data.call_deferred(node)

	for child in node.get("children", []):
		_add_nodes(child, positions)


func _connect_nodes(node: Dictionary) -> void:
	var parent_id: String = str(node.get("id", ""))
	for child in node.get("children", []):
		var child_id: String = str(child.get("id", ""))
		if has_node(child_id) and has_node(parent_id):
			connect_node(parent_id, 0, child_id, 0)
		_connect_nodes(child)
