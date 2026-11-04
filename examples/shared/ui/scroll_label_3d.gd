extends Label3D
## [Label3D] on a parchment scroll that faces the camera with it and resizes to the text.
## The scroll is an internal quad drawn with [member scroll_material]; hiding the label
## hides it too.

## Material for the scroll quad (parchment_scroll_3d.tres). Null draws no scroll.
@export var scroll_material: Material
## Space between the text and the scroll's edges, in metres.
@export var scroll_padding: Vector2 = Vector2(0.24, 0.1)

var _scroll: MeshInstance3D
var _quad: QuadMesh
var _fitted_text: String = ""


func _ready() -> void:
	if scroll_material == null:
		return
	_quad = QuadMesh.new()
	_scroll = MeshInstance3D.new()
	_scroll.mesh = _quad
	_scroll.material_override = scroll_material
	_scroll.cast_shadow = GeometryInstance3D.SHADOW_CASTING_SETTING_OFF
	add_child(_scroll, false, Node.INTERNAL_MODE_FRONT)


## Refits the scroll when the text changes. Subclasses call this from their own
## [code]_process[/code].
func _process(_delta: float) -> void:
	if _scroll == null or text == _fitted_text:
		return
	_fitted_text = text
	var used_font: Font = font if font != null else ThemeDB.fallback_font
	var text_size: Vector2 = (
		used_font.get_multiline_string_size(text, horizontal_alignment, -1.0, font_size)
		* pixel_size
	)
	var size: Vector2 = text_size + scroll_padding * 2.0
	# Follow the label's anchoring: bottom-aligned text sits above the origin, and so on.
	var center: Vector2 = offset * pixel_size
	match vertical_alignment:
		VERTICAL_ALIGNMENT_TOP:
			center.y -= text_size.y * 0.5
		VERTICAL_ALIGNMENT_BOTTOM:
			center.y += text_size.y * 0.5
	_quad.size = size
	# Slightly behind the text so the label always draws on top.
	_quad.center_offset = Vector3(center.x, center.y, -0.02)
	_scroll.set_instance_shader_parameter(&"scroll_size", size)
