// Billboarded parchment scroll drawn behind a Label3D. The quad's size comes in through the
// scroll_size instance uniform, set by scroll_label_3d.gd.
shader_type spatial;
render_mode unshaded, cull_disabled, shadows_disabled, fog_disabled;

#include "res://examples/shared/ui/parchment.gdshaderinc"

uniform float roll_width = 0.09;
instance uniform vec2 scroll_size = vec2(1.0, 0.5);

void vertex() {
	// Face the camera like the label it backs.
	MODELVIEW_MATRIX = VIEW_MATRIX * mat4(
		INV_VIEW_MATRIX[0], INV_VIEW_MATRIX[1], INV_VIEW_MATRIX[2], MODEL_MATRIX[3]
	);
}

void fragment() {
	vec4 scroll = parchment(UV * scroll_size, scroll_size, roll_width, 0.005);
	if (scroll.a < 0.5) {
		discard;
	}
	ALBEDO = scroll.rgb;
}
