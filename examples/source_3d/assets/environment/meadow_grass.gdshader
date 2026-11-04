// Grass blades for a MultiMesh: sway in gusting wind, root colour matches the ground below.
// The blade mesh stores height (0 root, 1 tip) in UV.y.
shader_type spatial;
render_mode cull_disabled, specular_disabled, world_vertex_coords;

#include "res://examples/source_3d/assets/environment/meadow.gdshaderinc"

uniform vec3 tip_tint : source_color = vec3(0.86, 0.9, 0.5);
uniform float tip_mix : hint_range(0.0, 1.0) = 0.35;
uniform float wind_strength = 0.14;
uniform float wind_speed = 1.7;
uniform vec2 wind_direction = vec2(0.8, 0.6);

varying vec3 root_pos;
varying float height;

void vertex() {
	root_pos = MODEL_MATRIX[3].xyz;
	height = UV.y;
	// Gusts travel across the field as broad noise bands; each blade adds a small own phase.
	float gust = texture(meadow_noise, root_pos.xz * 0.02 - wind_direction * TIME * 0.05).r;
	float sway = sin(TIME * wind_speed + dot(root_pos.xz, wind_direction) * 0.6) * 0.35 + gust * 1.3;
	VERTEX.xz += normalize(wind_direction) * sway * wind_strength * height * height;
	// Light blades like the ground so the field shades as one surface.
	NORMAL = vec3(0.0, 1.0, 0.0);
}

void fragment() {
	vec3 base = meadow_color(root_pos.xz) * 0.82;
	vec3 tip = mix(meadow_color(root_pos.xz), tip_tint, tip_mix);
	ALBEDO = mix(base, tip, height) * meadow_cloud_light(root_pos.xz, TIME);
	// Both faces face up, or back faces would turn dark.
	NORMAL = (VIEW_MATRIX * vec4(0.0, 1.0, 0.0, 0.0)).xyz;
	ROUGHNESS = 1.0;
}
