// Parchment scroll for a Control (a ColorRect behind the on-screen guide). Rolls run along the
// top and bottom; rect_size is the control's size in pixels, set by its owner.
shader_type canvas_item;

#include "res://examples/shared/ui/parchment.gdshaderinc"

uniform vec2 rect_size = vec2(200.0, 100.0);
uniform float roll_width = 14.0;

void fragment() {
	// parchment() rolls along x, so swap axes for top and bottom rolls.
	vec2 p = UV.yx * rect_size.yx;
	COLOR = parchment(p, rect_size.yx, roll_width, 1.0);
}
