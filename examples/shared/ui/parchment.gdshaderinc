// Procedural parchment scroll shared by the 3D labels and the on-screen guide.
// Coordinates: p runs over [0, size]; the paper rolls up at both ends of the x axis.

uniform vec3 paper_color : source_color = vec3(0.95, 0.87, 0.68);
uniform vec3 paper_edge_color : source_color = vec3(0.76, 0.58, 0.34);
uniform vec3 roll_color : source_color = vec3(0.86, 0.72, 0.5);
uniform vec3 trim_color : source_color = vec3(0.52, 0.3, 0.14);

float parchment_hash(vec2 p) {
	return fract(sin(dot(p, vec2(127.1, 311.7))) * 43758.5453);
}

float parchment_noise(vec2 p) {
	vec2 i = floor(p);
	vec2 f = fract(p);
	f = f * f * (3.0 - 2.0 * f);
	float a = parchment_hash(i);
	float b = parchment_hash(i + vec2(1.0, 0.0));
	float c = parchment_hash(i + vec2(0.0, 1.0));
	float d = parchment_hash(i + vec2(1.0, 1.0));
	return mix(mix(a, b, f.x), mix(c, d, f.x), f.y);
}

// Colour (rgb) and coverage (a) of the scroll at p. roll is the roll's width along x and
// unit the length of one "texel" of wobble (so 3D metres and UI pixels both work).
vec4 parchment(vec2 p, vec2 size, float roll, float unit) {
	float inset = roll * 0.28;
	// Rolls: full-height cylinders at both ends; the sheet sits a little inside them.
	float roll_end = min(p.x, size.x - p.x);
	if (roll_end < roll) {
		float t = roll_end / roll;
		float shade = 0.62 + 0.5 * sin(t * 3.14159);
		vec3 c = roll_color * shade;
		// Dark spiral line where the paper wraps.
		c = mix(c, trim_color, smoothstep(0.08, 0.0, abs(t - 0.55)) * 0.8);
		return vec4(c, 1.0);
	}
	// Sheet between the rolls, with a gently torn top and bottom.
	float wobble = (parchment_noise(vec2(p.x / unit * 0.35, 0.0)) - 0.5) * inset * 0.6;
	float top = inset + wobble;
	float bottom = size.y - inset - wobble * 0.7;
	if (p.y < top || p.y > bottom) {
		return vec4(0.0);
	}
	float edge = min(min(p.y - top, bottom - p.y), roll_end - roll);
	float stain = parchment_noise(p / unit * 0.08) * 0.6 + parchment_noise(p / unit * 0.3) * 0.4;
	vec3 c = mix(paper_color, paper_edge_color, smoothstep(0.55, 0.95, stain) * 0.45);
	c = mix(paper_edge_color, c, smoothstep(0.0, roll * 0.9, edge));
	// A thin inked border inside the edge.
	float line = abs(edge - roll * 0.35);
	c = mix(c, trim_color, smoothstep(unit * 1.6, unit * 0.4, line) * 0.6);
	return vec4(c, 1.0);
}
