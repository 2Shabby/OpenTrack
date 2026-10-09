extends Control

signal start_race
signal back

const TERRAIN_CONTROLS := {
	"Amplitude": ["amplitude", 1.0],
	"Wavelength": ["wavelength", 1.0],
	"Gradient": ["max_gradient", 0.01],
	"Blend": ["blend_distance", 1.0],
}
var _stage_ids: Array[String] = []
var _seed_valid := true

func _ready() -> void:
	Game.stage_catalog.refresh()
	%Players.max_value = Game.MAX_PLAYERS
	%Length.min_value = Game.MIN_LENGTH
	%Length.max_value = Game.MAX_LENGTH
	%Players.value = Game.player_count
	%Length.value = Game.stage_length
	%Seed.text = str(Game.seed_value)
	for vehicle_name in Game.VEHICLE_NAMES:
		%Vehicle.add_item(vehicle_name)
	%Vehicle.select(maxi(0, Game.VEHICLE_SCENES.find(Game.car_scene)))
	%Vehicle.item_selected.connect(func(index: int) -> void: Game.car_scene = Game.VEHICLE_SCENES[index])
	%Mode.add_item("Saved rally stages")
	%Mode.add_item("Procedural test")
	%Mode.select(Game.stage_mode)
	%Region.add_item("All regions")
	var regions: Array[String] = []
	for entry in Game.stage_catalog.entries:
		if not entry.region in regions:
			regions.append(entry.region)
	regions.sort()
	for region in regions:
		%Region.add_item(region)
		if region == Game.stage_region:
			%Region.select(%Region.item_count - 1)
	for label in ["Any length", "Under 10 km", "10–20 km", "20 km and longer"]:
		%LengthFilter.add_item(label)
	%LengthFilter.select(Game.stage_length_filter)
	for control_name: String in TERRAIN_CONTROLS:
		var binding: Array = TERRAIN_CONTROLS[control_name]
		var control: SpinBox = get_node("%" + control_name)
		control.value = Game.terrain_settings.get(binding[0]) / binding[1]
		control.value_changed.connect(func(value: float) -> void: Game.terrain_settings.set(binding[0], value * binding[1]))
	%Players.value_changed.connect(func(value: float) -> void: Game.player_count = int(value))
	%Length.value_changed.connect(func(value: float) -> void: Game.stage_length = int(value))
	%Seed.text_changed.connect(_validate_seed)
	%Seed.text_submitted.connect(func(_text: String) -> void: _start())
	%Mode.item_selected.connect(func(index: int) -> void:
		Game.stage_mode = index
		_show_mode()
	)
	%Region.item_selected.connect(func(index: int) -> void:
		Game.stage_region = %Region.get_item_text(index)
		_populate_stages()
	)
	%LengthFilter.item_selected.connect(func(index: int) -> void:
		Game.stage_length_filter = index
		_populate_stages()
	)
	%Stage.item_selected.connect(func(index: int) -> void:
		Game.selected_stage_id = _stage_ids[index]
		_show_stage_details()
	)
	%RandomStage.pressed.connect(_random_stage)
	%Start.pressed.connect(_start)
	%Back.pressed.connect(func() -> void: back.emit())
	_populate_stages()
	_show_mode()
	if not Game.setup_error.is_empty():
		%Error.text = Game.setup_error
		Game.setup_error = ""
	%Players.get_line_edit().grab_focus()

func _show_mode() -> void:
	var experimental: bool = Game.stage_mode == Game.StageMode.PROCEDURAL_TEST
	var settings := $Center/Panel/Column/Settings
	for name in ["Seed", "Length", "Amplitude", "Wavelength", "Gradient", "Blend"]:
		settings.get_node(name).visible = experimental
		settings.get_node(name + "Label").visible = experimental
	for name in ["Region", "LengthFilter", "Stage"]:
		settings.get_node(name).visible = not experimental
		settings.get_node(name + "Label").visible = not experimental
	%RandomStage.visible = not experimental
	%StageDetails.visible = not experimental
	_update_start()

func _populate_stages() -> void:
	%Stage.clear()
	_stage_ids = [""]
	var ids: Array[String] = Game.filtered_stage_ids()
	%Stage.add_item("Random saved stage (%d available)" % ids.size())
	for id in ids:
		var entry: Dictionary = Game.stage_catalog.entry_for(id)
		_stage_ids.append(id)
		%Stage.add_item("%s · %.1f km" % [entry.name, entry.length_m / 1000.0])
	var selected := _stage_ids.find(Game.selected_stage_id)
	if selected < 0:
		Game.selected_stage_id = ""
		selected = 0
	%Stage.select(selected)
	%RandomStage.disabled = ids.is_empty()
	_show_stage_details()
	_update_start()

func _random_stage() -> void:
	Game.selected_stage_id = Game.stage_catalog.draw(Game.filtered_stage_ids())
	%Stage.select(maxi(0, _stage_ids.find(Game.selected_stage_id)))
	_show_stage_details()

func _show_stage_details() -> void:
	var entry: Dictionary = Game.stage_catalog.entry_for(Game.selected_stage_id)
	if entry.is_empty():
		%StageDetails.text = "Complete routes · random selection without repeats.\nAll drivers share the chosen stage; retries keep the same road."
	else:
		%StageDetails.text = "%s · %s · %.2f km · %s\nFull route with saved road and pacenotes." % [entry.event, entry.region, entry.length_m / 1000.0, entry.surface]

func _update_start() -> void:
	if Game.stage_mode == Game.StageMode.PROCEDURAL_TEST:
		%Start.disabled = not _seed_valid
		%Error.text = "Enter a valid 64-bit integer seed." if not _seed_valid else ""
	else:
		%Start.disabled = Game.filtered_stage_ids().is_empty()
		%Error.text = Game.stage_catalog.error if Game.stage_catalog.entries.is_empty() else ("No stages match these filters." if %Start.disabled else "")

func _validate_seed(text: String) -> void:
	var clean := text.strip_edges()
	var valid := clean.is_valid_int()
	if valid:
		var canonical := str(clean.to_int())
		var digits := clean.trim_prefix("+").trim_prefix("-")
		while digits.length() > 1 and digits.begins_with("0"):
			digits = digits.substr(1)
		valid = canonical.trim_prefix("-") == digits and (clean.to_int() == 0 or canonical.begins_with("-") == clean.begins_with("-"))
	_seed_valid = valid
	if valid:
		Game.seed_value = clean.to_int()
	_update_start()

func _start() -> void:
	%Players.apply()
	if Game.stage_mode == Game.StageMode.PROCEDURAL_TEST:
		%Length.apply()
		for control_name: String in TERRAIN_CONTROLS:
			get_node("%" + control_name).apply()
		_validate_seed(%Seed.text)
	else:
		_update_start()
	if not %Start.disabled:
		start_race.emit()
