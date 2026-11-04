class_name ScrollLabel2D
extends Label
## 2D counterpart of [code]scroll_label_3d.gd[/code]: a [Label] on a parchment scroll that
## resizes to the text, drawn above the scene's sprites. The scroll is an internal
## [ColorRect] behind the text; hiding the label hides it too. Set the ink colour in the
## label's [LabelSettings] (the 3D labels use [code]Color(0.26, 0.15, 0.07)[/code]).

## Shader for the scroll.
const PARCHMENT_SHADER: Shader = preload("res://examples/shared/ui/parchment_scroll_ui.gdshader")

## Space between the text and the scroll's edges, in pixels.
@export var scroll_padding: Vector2 = Vector2(10.0, 10.0)
## Draw order above the scene, so sprites never cover the text.
@export var draw_z_index: int = 10

var _scroll: ColorRect
var _fitted: String = ""


func _ready() -> void:
	z_index = draw_z_index
	_scroll = ColorRect.new()
	_scroll.show_behind_parent = true
	_scroll.mouse_filter = Control.MOUSE_FILTER_IGNORE
	var paper: ShaderMaterial = ShaderMaterial.new()
	paper.shader = PARCHMENT_SHADER
	paper.set_shader_parameter(&"roll_width", 5.0)
	_scroll.material = paper
	add_child(_scroll, false, Node.INTERNAL_MODE_FRONT)
	_fit()


## Refits the scroll when the text or the label's size changes. Subclasses call this from
## their own [code]_process[/code].
func _process(_delta: float) -> void:
	_fit()


func _fit() -> void:
	if _scroll == null:
		return
	var key: String = "%s|%s" % [text, size]
	if key == _fitted:
		return
	_fitted = key
	_scroll.visible = not text.is_empty()
	var used_font: Font = get_theme_font(&"font")
	var font_size: int = get_theme_font_size(&"font_size")
	if label_settings != null:
		if label_settings.font != null:
			used_font = label_settings.font
		font_size = label_settings.font_size
	var text_size: Vector2 = used_font.get_multiline_string_size(
		text, horizontal_alignment, -1.0, font_size
	)
	# Place it where the label draws the text: by its alignment inside the label's rect.
	var origin: Vector2 = Vector2.ZERO
	match horizontal_alignment:
		HORIZONTAL_ALIGNMENT_CENTER:
			origin.x = (size.x - text_size.x) * 0.5
		HORIZONTAL_ALIGNMENT_RIGHT:
			origin.x = size.x - text_size.x
	match vertical_alignment:
		VERTICAL_ALIGNMENT_CENTER:
			origin.y = (size.y - text_size.y) * 0.5
		VERTICAL_ALIGNMENT_BOTTOM:
			origin.y = size.y - text_size.y
	_scroll.position = origin - scroll_padding
	_scroll.size = text_size + scroll_padding * 2.0
	(_scroll.material as ShaderMaterial).set_shader_parameter(&"rect_size", _scroll.size)
