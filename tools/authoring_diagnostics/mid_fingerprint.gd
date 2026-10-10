extends RefCounted

const BAKE_VERSION := "saved-world-v1"

static func compute(stage: RallyStage) -> String:
	var hash_context := HashingContext.new()
	hash_context.start(HashingContext.HASH_SHA256)
	hash_context.update((BAKE_VERSION + Engine.get_version_info()["string"] + stage.source_sha256).to_utf8_buffer())
	for path in ["res://tools/authoring_diagnostics/terrain_field_previous.gd", "res://scripts/terrain_mesher.gd", "res://scripts/terrain_sampler.gd", "res://scripts/road_shoulders.gd", "res://scripts/track_geometry.gd", "res://scripts/rally_stage.gd", "res://tools/native_bake/native_bake.cpp"]:
		hash_context.update(FileAccess.get_sha256(path).to_utf8_buffer())
	for value: Variant in [stage.centers, stage.headings, stage.left_edges, stage.right_edges, stage.road_normals, stage.features, stage.road_width, stage.seed_value, stage.terrain_settings.amplitude, stage.terrain_settings.wavelength, stage.terrain_settings.blend_distance, stage.terrain_settings.max_gradient]:
		hash_context.update(var_to_bytes(value))
	return hash_context.finish().hex_encode()
