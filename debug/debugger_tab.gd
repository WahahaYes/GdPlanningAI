@tool
class_name GdPAIDebuggerTab
extends PanelContainer
## Debugger UI panel for agent/plan visualization.

## Agent data.
var agents: Dictionary = { }
## More detailed agent information.
var agents_info: Dictionary = { }
## Currently selected agent ID.
var current_agent_id: int = -1
## UI HSplitContainer.
var split_container: HSplitContainer
## UI VBoxContainer.
var left_panel: VBoxContainer
## UI ItemList.
var agent_list: ItemList
## RichTextLabel.
var agent_stats: RichTextLabel
## GdPAIPlanGraphEdit.
var graph_edit: GdPAIPlanGraphEdit


# Override
## Initializes UI components and connections.
func _ready() -> void:
	_build_ui()
	agent_list.item_selected.connect(_on_item_selected)
	_update_agent_view()


## Registers a new agent in the debugger
func register_agent(agent_id: int, agent_name: String) -> void:
	agents[agent_id] = agent_name
	var item_index: int = agent_list.add_item(agent_name)
	agent_list.set_item_metadata(item_index, agent_id)
	if current_agent_id == -1:
		agent_list.select(item_index)
		_on_item_selected(item_index)


## Unregisters an agent from the debugger.
func unregister_agent(agent_id: int) -> void:
	if not agents.has(agent_id):
		return

	for i in range(agent_list.get_item_count()):
		if agent_list.get_item_metadata(i) == agent_id:
			agent_list.remove_item(i)
			break

	agents.erase(agent_id)
	agents_info.erase(agent_id)
	if current_agent_id == agent_id:
		current_agent_id = -1
		_update_agent_view()


## Updates agent information in the debugger.
func update_agent_info(agent_id: int, agent_info: Dictionary) -> void:
	agents_info[agent_id] = agent_info
	if current_agent_id == agent_id:
		_update_agent_view()


## Clears all debugger state and UI
func clear_state() -> void:
	agents = { }
	agents_info = { }
	current_agent_id = -1
	agent_list.clear()
	_update_agent_view()


## Handles agent selection in the UI.
func _on_item_selected(index: int) -> void:
	if index < 0 or index >= agent_list.get_item_count():
		return

	var agent_id: int = int(agent_list.get_item_metadata(index))
	current_agent_id = agent_id
	_update_agent_view()


## Updates the agent view in the UI.
func _update_agent_view() -> void:
	if current_agent_id == -1 or not agents.has(current_agent_id):
		agent_stats.text = "No agent selected"
		graph_edit.plan_tree = { }
		return
	# Format the text box description.
	agent_stats.text = "[b]%s[/b]" % [agents[current_agent_id]]
	if not agents_info.has(current_agent_id):
		agent_stats.text += "\nNo info available"
		graph_edit.plan_tree = { }
		return
	if agents_info[current_agent_id].has("current_goal"):
		agent_stats.text += "\nCurrent goal: %s" % [agents_info[current_agent_id]["current_goal"]]
	# Add plan tree.
	if agents_info[current_agent_id].has("plan_tree"):
		graph_edit.plan_tree = agents_info[current_agent_id]["plan_tree"]
	else:
		graph_edit.plan_tree = { }


## Builds the UI components at startup.
func _build_ui() -> void:
	# Main split container
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
	agent_stats.bbcode_enabled = true
	left_panel.add_child(agent_stats)
	# Right panel (graph edit)
	graph_edit = GdPAIPlanGraphEdit.new()
	graph_edit.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	graph_edit.size_flags_vertical = Control.SIZE_EXPAND_FILL
	graph_edit.plan_tree = { }
	split_container.add_child(graph_edit)
