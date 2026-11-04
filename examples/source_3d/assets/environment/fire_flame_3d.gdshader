// Low-poly flame tongue: an unshaded cone that sways, twists and pulses. Colours above 1 let
// the environment's glow bloom it. Each flame sets its own flame_color and phase.
shader_type spatial;
render_mode unshaded, shadows_disabled, fog_disabled;

uniform float energy = 2.2;
uniform float speed = 7.0;
instance uniform vec3 flame_color : source_color = vec3(1.0, 0.5, 0.1);
instance uniform float phase = 0.0;

void vertex() {
	// CylinderMesh side UVs run from 0 at the top to 1 at the bottom.
	float h = clamp(1.0 - UV.y, 0.0, 1.0);
	float t = TIME * speed + phase;
	VERTEX.x += sin(t + VERTEX.y * 5.0) * 0.06 * h;
	VERTEX.z += cos(t * 1.3 + phase) * 0.06 * h;
	VERTEX.y *= 1.0 + 0.18 * sin(t * 1.7) * h;
}

void fragment() {
	ALBEDO = flame_color * energy;
}
