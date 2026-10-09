class_name StageCatalog
extends RefCounted

const INDEX_PATH := "res://resources/stages/catalog.tres"


var entries: Array[Dictionary] = []
var error := ""
var _bag: Array[String] = []
var _eligible: Array[String] = []
var _rng := RandomNumberGenerator.new()
var _last_id := ""

func _init() -> void:
	_rng.randomize()
	refresh()

func refresh() -> void:
	entries.clear()
	error = ""
	var index := ResourceLoader.load(INDEX_PATH, "", ResourceLoader.CACHE_MODE_IGNORE)
	if index == null:
		error = "The saved stage catalog is missing."
		return
	var data := {"version": index.get_meta("version", 0), "stages": index.get_meta("stages", [])}
	if not data is Dictionary or data.get("version") != 1 or not data.get("stages") is Array:
		error = "The saved stage catalog is invalid."
		return
	var seen := {}
	for entry: Variant in data.stages:
		if not entry is Dictionary or not entry.get("id") is String or not entry.get("name") is String or not entry.get("path") is String or (not entry.get("length_m") is float and not entry.get("length_m") is int):
			error = "The saved stage catalog contains an invalid entry."
			entries.clear()
			return
		if seen.has(entry.id) or not entry.path.begins_with("res://resources/stages/") or not ResourceLoader.exists(entry.path):
			error = "The saved stage catalog contains a duplicate or missing stage."
			entries.clear()
			return
		seen[entry.id] = true
		entries.append(entry)
	if entries.is_empty():
		error = "The saved stage catalog is empty."

func entry_for(id: String) -> Dictionary:
	for entry in entries:
		if entry.id == id:
			return entry
	return {}

# Draw without replacement. Retries and hotseat handoffs never draw a stage.
# The optional IDs let the same bag support length/region filters in setup.
func draw(ids: Array[String]) -> String:
	if ids.is_empty():
		return ""
	if ids != _eligible:
		_eligible.assign(ids)
		_bag.clear()
	var allowed := {}
	for id in ids:
		allowed[id] = true
	_bag = _bag.filter(func(id: String) -> bool: return allowed.has(id))
	if _bag.is_empty():
		_bag.assign(ids)
		for i in range(_bag.size() - 1, 0, -1):
			var j := _rng.randi_range(0, i)
			var swap := _bag[i]
			_bag[i] = _bag[j]
			_bag[j] = swap
		if _bag.size() > 1 and _bag.back() == _last_id:
			var swap := _bag[0]
			_bag[0] = _bag[-1]
			_bag[-1] = swap
	_last_id = _bag.pop_back()
	return _last_id

func load_stage(id: String) -> RallyStage:
	error = ""
	var entry := entry_for(id)
	if entry.is_empty():
		error = "The selected saved stage was not found."
		return null
	# Fresh resources isolate mutable terrain/build state between sessions.
	var stage := ResourceLoader.load(entry.path, "", ResourceLoader.CACHE_MODE_IGNORE) as RallyStage
	if stage == null or stage.stage_id != id or stage.centers.size() < 2 or stage.terrain_settings == null or not stage.terrain_settings.valid():
		error = "The selected saved stage is invalid."
		return null
	if stage.baked_scene_path.is_empty() or not ResourceLoader.exists(stage.baked_scene_path):
		error = "This stage's saved world is unavailable."
		return null
	return stage
