// Additive, camera-facing flame puff for GPUParticles3D. The particle colour (from the process
// material's colour ramp) is boosted past 1 so the environment's glow blooms it.
shader_type spatial;
render_mode blend_add, unshaded, cull_disabled, depth_draw_never, shadows_disabled, fog_disabled;

uniform float energy = 0.9;

void vertex() {
	mat4 billboard = mat4(
		normalize(INV_VIEW_MATRIX[0]), normalize(INV_VIEW_MATRIX[1]),
		normalize(INV_VIEW_MATRIX[2]), MODEL_MATRIX[3]
	);
	float s = length(MODEL_MATRIX[0].xyz);
	MODELVIEW_MATRIX = VIEW_MATRIX * billboard * mat4(
		vec4(s, 0.0, 0.0, 0.0), vec4(0.0, s, 0.0, 0.0), vec4(0.0, 0.0, s, 0.0), vec4(0.0, 0.0, 0.0, 1.0)
	);
}

void fragment() {
	// Teardrop: round at the bottom, pinched towards the top.
	vec2 p = UV - vec2(0.5, 0.62);
	p.x /= mix(1.0, 0.35, clamp(-p.y * 2.0, 0.0, 1.0));
	float shape = smoothstep(0.42, 0.12, length(p));
	ALBEDO = COLOR.rgb * energy * shape * COLOR.a;
}
