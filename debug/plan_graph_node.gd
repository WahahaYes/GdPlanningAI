@tool
class_name GdPAIPlanGraphNode
extends GraphNode
## Custom GraphNode for visualizing planning actions.

## Port color.
const PORT_COLOR := Color(0.8, 0.8, 0.8, 0.1)

## Title label at top of the GraphNode.
var title_label: Label
## Contents displayed in the GraphNode body.
var body_label: RichTextLabel


# Override
## Initializes node UI components.
func _ready() -> void:
	draggable = false
	set_slot(0, true, TYPE_INT, PORT_COLOR, true, TYPE_INT, PORT_COLOR)

	var body_container := VBoxContainer.new()
	body_container.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	body_container.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body_container.custom_minimum_size = Vector2(160, 80)
	add_child(body_container)

	title_label = Label.new()
	title_label.horizontal_alignment = HORIZONTAL_ALIGNMENT_CENTER
	title_label.vertical_alignment = VERTICAL_ALIGNMENT_CENTER
	title_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	title_label.add_theme_color_override("font_color", Color(0.1, 0.1, 0.15))
	body_container.add_child(title_label)

	body_label = RichTextLabel.new()
	body_label.fit_content = true
	body_label.scroll_active = false
	body_label.bbcode_enabled = true
	body_label.autowrap_mode = TextServer.AUTOWRAP_WORD
	body_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	body_label.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body_label.add_theme_color_override("default_color", Color(0.2, 0.2, 0.25))
	body_label.hide() # Hidden until content provided
	body_container.add_child(body_label)


## Updates the node's data display.
func set_node_data(node_data: Dictionary) -> void:
	var title_text: String = node_data.get("action", "Plan Node")
	title = title_text
	title_label.text = title_text

	# Set color based on runtime status
	var status_color: Color
	if node_data.has("runtime_status"):
		match node_data["runtime_status"]:
			"...":
				status_color = Color.GRAY
			Action.Status.RUNNING:
				status_color = Color.YELLOW
			Action.Status.SUCCESS:
				status_color = Color.GREEN
			Action.Status.FAILURE:
				status_color = Color.RED
			_:
				status_color = Color.NAVY_BLUE
	else:
		status_color = Color.NAVY_BLUE

	_update_theme(status_color)

	var details: Array = []
	if node_data.has("cost"):
		details.append("[b]Cost:[/b] %.2f" % float(node_data["cost"]))

	# Add status information if available
	if node_data.has("pre_status"):
		details.append("[b]Pre:[/b] %s" % node_data["pre_status"])
	if node_data.has("runtime_status"):
		details.append("[b]Exec:[/b] %s" % node_data["runtime_status"])
	if node_data.has("post_status"):
		details.append("[b]Post:[/b] %s" % node_data["post_status"])

	# Add other debug info
	if node_data.has("desired_state_count") and node_data["desired_state_count"] > 0:
		details.append("[b]Preconditions:[/b] %d" % node_data["desired_state_count"])

	body_label.bbcode_text = "\n".join(details)
	body_label.visible = not details.is_empty()


## Updates colors based on node status.
func _update_theme(status_color: Color = Color.GRAY) -> void:
	var style := StyleBoxFlat.new()
	style.bg_color = Color(0.13, 0.14, 0.21).lerp(status_color, 0.3)
	style.border_width_bottom = 2
	style.border_width_top = 2
	style.border_width_left = 2
	style.border_width_right = 2
	style.border_color = Color(0.36, 0.52, 0.96).lerp(status_color, 0.7)
	add_theme_stylebox_override("panel", style)
