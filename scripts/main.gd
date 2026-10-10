extends Node

var content: Node

func _ready() -> void:
	_show_menu()

func _show_menu() -> void:
	var menu := _replace_content(preload("res://scenes/ui/main_menu.tscn"), Game.State.MAIN_MENU)
	menu.start_hotseat.connect(_show_setup)
	menu.quit_game.connect(func() -> void: get_tree().quit())

func _show_setup() -> void:
	var setup := _replace_content(preload("res://scenes/ui/setup_menu.tscn"), Game.State.SETUP)
	setup.start_race.connect(_start_race)
	setup.back.connect(_show_menu)

func _start_race(authored_stage: RallyStage = null) -> void:
	Game.configure_players(Game.player_count)
	var world := _replace_content(preload("res://scenes/world.tscn"), Game.State.DRIVING)
	world.open_setup.connect(_show_setup)
	world.open_menu.connect(_show_menu)
	if not await world.start_race(authored_stage):
		_show_setup()

func _replace_content(scene: PackedScene, state: int) -> Node:
	if is_instance_valid(content):
		remove_child(content)
		content.queue_free()
	get_tree().paused = false
	Game.state = state
	Game.paused = false
	content = scene.instantiate()
	add_child(content)
	return content
