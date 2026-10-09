extends SceneTree

const Stage := preload("res://scripts/rally_stage.gd")
const Terrain := preload("res://scripts/terrain_builder.gd")

func _initialize() -> void:
	_bake.call_deferred()

func _bake() -> void:
	var data: Variant = JSON.parse_string(FileAccess.get_file_as_string("res://assets/tracks/recipes.json"))
	if not data is Dictionary or data.get("version") != 1:
		push_error("Run python3 tools/author_stages.py first.")
		quit(1)
		return
	var entries: Array[Dictionary] = []
	for recipe: Dictionary in data.stages:
		var stage := Stage.new()
		stage.generator_version = "rally2gpx-authored-v1"
		stage.stage_id = recipe.id
		stage.display_name = recipe.name
		stage.region = recipe.region
		stage.source_url = recipe.source_url
		stage.source_gpx = "res://" + recipe.gpx
		stage.source_sha256 = recipe.source_sha256
		stage.source_length_m = recipe.source_length_m
		stage.requested_length_m = roundi(recipe.source_length_m)
		stage.road_width = recipe.road_width
		stage.seed_value = int(recipe.seed)
		for point: Array in recipe.centers:
			stage.centers.append(Vector3(point[0], point[1], point[2]))
		stage.headings = PackedFloat64Array(recipe.headings)
		stage.distances = PackedFloat64Array(recipe.distances)
		for feature: Dictionary in recipe.features:
			feature.first_station = int(feature.first_station)
			feature.last_station = int(feature.last_station)
			feature.grade = int(feature.grade)
			feature.surface = int(feature.surface)
			stage.features.append(feature)
		var builder := Terrain.new()
		if not builder.apply(stage, preload("res://resources/terrain_settings.tres")):
			push_error(recipe.id + ": " + builder.error)
			quit(1)
			return
		# Save geometry and settings; the runtime field is recreated on load.
		stage.terrain = null
		var path: String = "res://resources/stages/" + recipe.id + ".res"
		var result := ResourceSaver.save(stage, path, ResourceSaver.FLAG_COMPRESS)
		if result != OK:
			push_error("Could not save " + path)
			quit(1)
			return
		entries.append({"id": stage.stage_id, "name": stage.display_name, "region": stage.region,
			"event": recipe.event, "path": path, "length_m": stage.length_m,
			"source_length_m": stage.source_length_m, "surface": "Asphalt" if recipe.surface == 0 else "Dirt",
			"source_url": stage.source_url, "road_width": stage.road_width, "baked_scene_path": ""})
		print("Saved ", stage.stage_id, " · ", roundi(stage.length_m), " m")
	var file := FileAccess.open("res://resources/stages/catalog.json", FileAccess.WRITE)
	file.store_string(JSON.stringify({"version": 1, "stages": entries}, "\t") + "\n")
	var index := Resource.new()
	index.set_meta("version", 1)
	index.set_meta("stages", entries)
	ResourceSaver.save(index, "res://resources/stages/catalog.tres")
	print("Saved catalog: ", entries.size(), " complete stages")
	quit()
