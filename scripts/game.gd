extends Node

enum State { MAIN_MENU, SETUP, DRIVING }
const StageGenerator := preload("res://scripts/rally_generator.gd")
const MAX_PLAYERS := 16
const MIN_LENGTH := StageGenerator.MIN_LENGTH
const MAX_LENGTH := StageGenerator.MAX_LENGTH
const VEHICLE_NAMES := ["Rally hatchback", "Auto rickshaw · slow"]
const VEHICLE_SCENES := [preload("res://scenes/cars/rally_hatchback.tscn"), preload("res://scenes/cars/auto_rickshaw.tscn")]

var state: int = State.MAIN_MENU
var paused: bool = false
var players: PackedStringArray = PackedStringArray(["Driver 1", "Driver 2"])
var player_index: int = 0
var seed_value: int = 1592598566
var stage_length: int = 1200
var terrain_settings: Resource = preload("res://resources/terrain_settings.tres").duplicate()
var player_count: int = 2
var setup_error := ""
var car_scene: PackedScene = VEHICLE_SCENES[0]

func player_paint_index(index: int) -> int:
	return Palette.player_index(index)

func player_name() -> String:
	if players.is_empty():
		return "Driver 1"
	return players[player_index]

func configure_players(count: int) -> void:
	player_count = clampi(count, 1, MAX_PLAYERS)
	players = PackedStringArray()
	for index in player_count:
		players.append("Driver %d" % (index + 1))
	player_index = 0

func next_driver() -> void:
	if players.is_empty():
		return
	player_index = (player_index + 1) % players.size()

func add_driver() -> void:
	if players.size() >= MAX_PLAYERS:
		return
	players.append("Driver %d" % (players.size() + 1))
	player_count = players.size()

func create_stage() -> RallyStage:
	var generator := StageGenerator.new()
	var result: RallyStage = generator.generate(seed_value, stage_length, terrain_settings)
	setup_error = generator.error
	return result
