@tool
class_name GdPlanningAIPlanGraphNode
extends GraphNode

const PORT_COLOR := Color(0.8, 0.8, 0.8)

var title_label: Label
var body_label: RichTextLabel


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
	title_label.add_theme_color_override("font_color", Color(0.95, 0.95, 1.0))
	body_container.add_child(title_label)

	body_label = RichTextLabel.new()
	body_label.fit_content = true
	body_label.scroll_active = false
	body_label.bbcode_enabled = true
	body_label.autowrap_mode = TextServer.AUTOWRAP_WORD
	body_label.size_flags_horizontal = Control.SIZE_EXPAND_FILL
	body_label.size_flags_vertical = Control.SIZE_EXPAND_FILL
	body_label.hide() # Hidden until content provided
	body_container.add_child(body_label)

	_update_theme()


func set_node_data(node_data: Dictionary) -> void:
	var title_text: String = node_data.get("action", "Plan Node")
	title = title_text
	title_label.text = title_text

	var details := []
	if node_data.has("cost"):
		details.append("[b]Cost:[/b] %.2f" % float(node_data["cost"]))
	if node_data.has("details") and not node_data["details"].is_empty():
		details.append(node_data["details"])
	if node_data.has("desired_state_count") and node_data["desired_state_count"] > 0:
		details.append("[b]Preconditions:[/b] %d" % node_data["desired_state_count"])

	var has_details := not details.is_empty()
	body_label.visible = has_details
	if has_details:
		body_label.bbcode_text = "\n".join(details)
	else:
		body_label.bbcode_text = ""


func _update_theme() -> void:
	var style := StyleBoxFlat.new()
	style.bg_color = Color(0.13, 0.14, 0.21)
	style.border_width_bottom = 2
	style.border_width_top = 2
	style.border_width_left = 2
	style.border_width_right = 2
	style.border_color = Color(0.36, 0.52, 0.96)
	add_theme_stylebox_override("panel", style)
