// Meadow colouring shared by the ground and the grass blades, so blade roots match the soil
// under them. Everything is in world space: neighbouring meshes line up without seams.

uniform vec3 meadow_light : source_color = vec3(0.5, 0.64, 0.3);
uniform vec3 meadow_dark : source_color = vec3(0.3, 0.47, 0.22);
uniform vec3 meadow_dry : source_color = vec3(0.7, 0.66, 0.38);
// Seamless noise, sampled at several scales for patches, detail and cloud shadows.
uniform sampler2D meadow_noise : repeat_enable, filter_linear_mipmap;
uniform float patch_scale = 0.03;
uniform float dry_amount : hint_range(0.0, 1.0) = 0.45;
uniform vec2 cloud_velocity = vec2(0.8, 0.35);
uniform float cloud_scale = 0.012;
uniform float cloud_shadow : hint_range(0.0, 1.0) = 0.22;

// Base meadow colour at a world position: light and dark patches with drier spots.
vec3 meadow_color(vec2 p) {
	float patches = texture(meadow_noise, p * patch_scale).r;
	float detail = texture(meadow_noise, p * patch_scale * 9.0 + 0.37).r;
	vec3 grass = mix(meadow_dark, meadow_light, smoothstep(0.3, 0.7, patches * 0.7 + detail * 0.3));
	float dry = smoothstep(0.62, 0.8, texture(meadow_noise, p * patch_scale * 0.5 + 0.71).r);
	return mix(grass, meadow_dry, dry * dry_amount);
}

// Soft shadows of passing clouds, 1 where lit. Animated by TIME.
float meadow_cloud_light(vec2 p, float time) {
	float c = texture(meadow_noise, p * cloud_scale + cloud_velocity * cloud_scale * time).r;
	return 1.0 - cloud_shadow * smoothstep(0.5, 0.68, c);
}
