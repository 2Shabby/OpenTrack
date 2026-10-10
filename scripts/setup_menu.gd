extends Control

signal start_race
signal back

const TERRAIN_CONTROLS := {
	"Amplitude": ["amplitude", 1.0],
	"Wavelength": ["wavelength", 1.0],
	"Gradient": ["max_gradient", 0.01],
	"Blend": ["blend_distance", 1.0],
}
var _seed_valid := true

func _ready() -> void:
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
	for control_name: String in TERRAIN_CONTROLS:
		var binding: Array = TERRAIN_CONTROLS[control_name]
		var control: SpinBox = get_node("%" + control_name)
		control.value = Game.terrain_settings.get(binding[0]) / binding[1]
		control.value_changed.connect(func(value: float) -> void: Game.terrain_settings.set(binding[0], value * binding[1]))
	%Players.value_changed.connect(func(value: float) -> void: Game.player_count = int(value))
	%Length.value_changed.connect(func(value: float) -> void: Game.stage_length = int(value))
	%Seed.text_changed.connect(_validate_seed)
	%Seed.text_submitted.connect(func(_text: String) -> void: _start())
	%Start.pressed.connect(_start)
	%Back.pressed.connect(func() -> void: back.emit())
	_validate_seed(%Seed.text)
	if not Game.setup_error.is_empty():
		%Error.text = Game.setup_error
		Game.setup_error = ""
	%Players.get_line_edit().grab_focus()

func _update_start() -> void:
	%Start.disabled = not _seed_valid
	%Error.text = "Enter a valid 64-bit integer seed." if not _seed_valid else ""

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
	%Length.apply()
	for control_name: String in TERRAIN_CONTROLS:
		get_node("%" + control_name).apply()
	_validate_seed(%Seed.text)
	if not %Start.disabled:
		start_race.emit()
