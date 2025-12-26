@tool
class_name GdPAIPlanGraphEdit
extends GraphEdit
## Interactive graph editor for plan visualization.

## Horizontal spacing between nodes.
const H_SPACING: float = 300.0
## Vertical spacing between nodes.
const V_SPACING: float = 350.0

## Plan tree data.
var plan_tree: Dictionary:
	set(value):
		if _plan_tree == value:
			return
		_plan_tree = value
		_update_graph()
	get:
		return _plan_tree
## Internal plan tree data.
var _plan_tree: Dictionary = { }


# Override
func _ready() -> void:
	show_arrange_button = false
	minimap_enabled = false
	grid_pattern = GraphEdit.GRID_PATTERN_DOTS


## Rebuilds the entire graph visualization
## Clears existing nodes and recreates them based on current plan_tree
func _update_graph() -> void:
	# TODO: If ever needed could use object pooling instead of a full rebuild every update.
	var levels: Dictionary = { }
	var positions: Dictionary = { }

	clear_connections()

	for child in get_children():
		if child is GraphNode:
			remove_child(child)
			child.queue_free()

	if _plan_tree.is_empty():
		return

	# First pass: Collect nodes by depth
	_collect_nodes_by_depth(_plan_tree, 0, levels)
	# Second pass: Assign positions
	_assign_positions(levels, positions)

	_add_nodes(_plan_tree, positions)
	_connect_nodes(_plan_tree)


## Organizes nodes by their depth in the plan tree.
func _collect_nodes_by_depth(
		node: Dictionary,
		depth: int,
		levels: Dictionary,
) -> void:
	if not levels.has(depth):
		levels[depth] = []
	levels[depth].append(node)

	for child in node.get("children", []):
		_collect_nodes_by_depth(child, depth + 1, levels)


## Calculates and assigns positions to all nodes for graph layout.
func _assign_positions(
		levels: Dictionary,
		positions: Dictionary,
) -> void:
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


## Creates and positions graph nodes based on the calculated layout.
func _add_nodes(
		node: Dictionary,
		positions: Dictionary,
) -> void:
	var graph_node: GdPAIPlanGraphNode = GdPAIPlanGraphNode.new()
	graph_node.name = str(node.get("id", ""))
	add_child(graph_node)
	graph_node.position_offset = positions.get(graph_node.name, Vector2.ZERO)
	graph_node.set_node_data.call_deferred(node)

	for child in node.get("children", []):
		_add_nodes(child, positions)


## Creates connections between parent and child nodes in the graph.
func _connect_nodes(node: Dictionary) -> void:
	var parent_id: String = str(node.get("id", ""))
	for child in node.get("children", []):
		var child_id: String = str(child.get("id", ""))
		if has_node(child_id) and has_node(parent_id):
			connect_node(parent_id, 0, child_id, 0)
		_connect_nodes(child)
