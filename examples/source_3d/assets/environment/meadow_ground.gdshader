// Stylized meadow ground: patchy grass colour with drifting cloud shadows.
shader_type spatial;
render_mode specular_disabled;

#include "res://examples/source_3d/assets/environment/meadow.gdshaderinc"

varying vec3 world_pos;

void vertex() {
	world_pos = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	// The soil between blades reads slightly darker than the blades themselves.
	ALBEDO = meadow_color(world_pos.xz) * 0.82 * meadow_cloud_light(world_pos.xz, TIME);
	ROUGHNESS = 1.0;
}
