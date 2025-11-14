@tool
class_name GdPlanningAIPlanGraphEdit
extends GraphEdit

const H_SPACING := 300.0
const V_SPACING := 350.0

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

	# First pass: Collect nodes by depth
	var levels := { }
	_collect_nodes_by_depth(_plan_tree, 0, levels)

	# Second pass: Assign positions
	var positions := { }
	_assign_positions(levels, positions)

	_add_nodes(_plan_tree, positions)
	_connect_nodes(_plan_tree)


func _collect_nodes_by_depth(node: Dictionary, depth: int, levels: Dictionary) -> void:
	if not levels.has(depth):
		levels[depth] = []
	levels[depth].append(node)

	for child in node.get("children", []):
		_collect_nodes_by_depth(child, depth + 1, levels)


func _assign_positions(levels: Dictionary, positions: Dictionary) -> void:
	# Root node at 0,0
	if levels.has(0) and levels[0].size() > 0:
		positions[levels[0][0].get("id")] = Vector2.ZERO

	# Position other levels
	for depth in levels.keys():
		if depth == 0:
			continue

		var nodes_at_level = levels[depth]
		var level_width: float = (nodes_at_level.size() - 1) * H_SPACING
		var start_x: float = -level_width / 2

		for i in range(nodes_at_level.size()):
			var node = nodes_at_level[i]
			positions[node.get("id")] = Vector2(start_x + i * H_SPACING, depth * V_SPACING)


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
