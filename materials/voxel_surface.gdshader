shader_type spatial;

uniform sampler2D surface_texture : source_color, filter_nearest, repeat_enable;
uniform sampler2D palette_texture : source_color, filter_nearest, repeat_disable;
uniform int surface_kind = 2;
varying vec3 world_position;

void vertex() {
	world_position = (MODEL_MATRIX * vec4(VERTEX, 1.0)).xyz;
}

void fragment() {
	vec2 cell = floor(world_position.xz / 0.1);
	ALBEDO = texture(surface_texture, (cell + 0.5) / 32.0).rgb;
	ROUGHNESS = surface_kind == 0 ? 0.92 : 1.0;
	if (surface_kind == 0) {
		if (abs(UV.x - 0.5) < 0.005 && mod(UV.y, 6.0) < 3.0) {
			ALBEDO = texture(palette_texture, vec2(2.5 / 32.0, 0.5)).rgb;
		}
		if (UV.x < 0.015 || UV.x > 0.985) {
			ALBEDO = texture(palette_texture, vec2(19.5 / 32.0, 0.5)).rgb;
		}
	}
}
