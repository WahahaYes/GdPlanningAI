@tool
class_name GdPlanningAIDebuggerTab
extends PanelContainer

# Agent data
var agents: Dictionary = {}
var agent_plans: Dictionary = {}
var current_agent_id := -1
# UI elements
var split_container: HSplitContainer
var left_panel: VBoxContainer
var agent_list: ItemList
var agent_stats: RichTextLabel
var graph_edit: GdPlanningAIPlanGraphEdit


func _ready() -> void:
	_build_ui()
	agent_list.item_selected.connect(_on_item_selected)
	_update_agent_view()


func register_agent(agent_id: int, agent_name: String) -> void:
	agents[agent_id] = agent_name
	var item_index := agent_list.add_item(agent_name)
	agent_list.set_item_metadata(item_index, agent_id)
	if current_agent_id == -1:
		agent_list.select(item_index)
		_on_item_selected(item_index)


func unregister_agent(agent_id: int) -> void:
	if not agents.has(agent_id):
		return

	for i in range(agent_list.get_item_count()):
		if agent_list.get_item_metadata(i) == agent_id:
			agent_list.remove_item(i)
			break

	agents.erase(agent_id)
	agent_plans.erase(agent_id)
	if current_agent_id == agent_id:
		current_agent_id = -1
		_update_agent_view()


func _on_item_selected(index: int) -> void:
	if index < 0 or index >= agent_list.get_item_count():
		return

	var agent_id: int = int(agent_list.get_item_metadata(index))
	current_agent_id = agent_id
	_update_agent_view()


func _update_agent_view() -> void:
	if current_agent_id == -1 or not agents.has(current_agent_id):
		agent_stats.text = "No agent selected"
		graph_edit.plan_tree = {}
		return

	agent_stats.text = "[b]%s[/b]\nID: %d" % [agents[current_agent_id], current_agent_id]
	if agent_plans.has(current_agent_id):
		graph_edit.plan_tree = agent_plans[current_agent_id]
	else:
		graph_edit.plan_tree = {}


func update_plan(agent_id: int, plan_tree: Dictionary) -> void:
	agent_plans[agent_id] = plan_tree
	if current_agent_id == agent_id:
		graph_edit.plan_tree = plan_tree


func _build_ui() -> void:
	# Main split container (20% left, 80% right)
	split_container = HSplitContainer.new()
	split_container.set_anchors_and_offsets_preset(Control.PRESET_FULL_RECT)
	add_child(split_container)

	# Left panel (agent selection + stats)
	left_panel = VBoxContainer.new()
	left_panel.custom_minimum_size = Vector2(200, 0) # Minimum width
	split_container.add_child(left_panel)

	# Agent list
	agent_list = ItemList.new()
	agent_list.size_flags_vertical = Control.SIZE_EXPAND_FILL
	left_panel.add_child(agent_list)

	# Agent stats
	agent_stats = RichTextLabel.new()
	agent_stats.custom_minimum_size = Vector2(0, 100)
	left_panel.add_child(agent_stats)

	# Right panel (graph edit)
	graph_edit = GdPlanningAIPlanGraphEdit.new()
	graph_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	graph_edit.size_flags_vertical = Control.SIZE_EXPAND_FILL
	graph_edit.plan_tree = {}
	split_container.add_child(graph_edit)
