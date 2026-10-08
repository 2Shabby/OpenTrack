extends Node3D

signal open_setup
signal open_menu

const FALL_MARGIN := 30.0

var car_root: RallyCar
var track_info: Dictionary = {}
var _telemetry: Dictionary = {}
var stage: Resource
var _station := 0
var _progress := 0.0
var _elapsed := 0.0
var _started := false
var _finished := false
var _best_times: Dictionary = {}
var _spawn_pose := Transform3D.IDENTITY
var _previous_position := Vector3.ZERO
var recovery_count := 0

@onready var camera: Camera3D = $ChaseCamera
@onready var pause_menu: CanvasLayer = $PauseMenu
@onready var debug_label: Label = $Debug/Label
@onready var hud: Control = $HUD/RallyHUD

func start_race(seed_value: int, length_m: int) -> bool:
	var generator := Game.StageGenerator.new()
	stage = generator.generate(seed_value, length_m, Game.terrain_settings)
	if stage == null:
		Game.setup_error = generator.error
		return false
	var geometry := TrackGeometry.new()
	geometry.name = "Track"
	add_child(geometry)
	geometry.build(stage)
	_spawn_pose = stage.spawn_pose()
	if not _spawn_car():
		return false
	track_info = stage.info()
	_reset_run()
	debug_label.text = "Track seed %s" % track_info.get("seed", 0)
	pause_menu.toggle_pause.connect(_toggle_pause)
	pause_menu.resume_race.connect(_resume)
	pause_menu.restart_race.connect(_restart)
	pause_menu.next_driver.connect(_next_driver)
	pause_menu.open_setup.connect(func() -> void: open_setup.emit())
	pause_menu.open_menu.connect(func() -> void: open_menu.emit())
	pause_menu.quit_game.connect(func() -> void: get_tree().quit())
	_update_hud()
	return true

func set_paused(value: bool) -> void:
	Game.paused = value
	get_tree().paused = value
	pause_menu.visible = value
	if value and car_root:
		car_root.clear_held_input()

func _physics_process(delta: float) -> void:
	if Game.state != Game.State.DRIVING or car_root == null or Game.paused:
		return
	if Input.is_action_just_pressed("reset_car"):
		_restart_car()
		return
	if _finished:
		return
	if _out_of_world(car_root.global_position):
		recovery_count += 1
		_restart_car()
		return
	var current := car_root.global_position
	_started = _started or car_root.drive_requested()
	if _started:
		_elapsed += delta
	if _started and stage.crossed_finish(_previous_position, current):
		_finish()
	_previous_position = current
	_telemetry = car_root.telemetry()
	var projected: Dictionary = stage.project_distance(current, _station)
	_station = projected["station"]
	_progress = projected["distance"]
	_update_hud()

func _process(delta: float) -> void:
	if Input.is_action_just_pressed("toggle_debug"):
		debug_label.visible = not debug_label.visible
	if Input.is_action_just_pressed("next_driver") and not Game.paused and car_root:
		_next_driver()
	if Input.is_action_just_pressed("add_driver") and not Game.paused:
		Game.add_driver()
		_update_hud()
	if Game.state == Game.State.DRIVING and car_root and not Game.paused:
		camera.follow(car_root, car_root.linear_velocity, delta)
		if debug_label.visible:
			debug_label.text = _debug_text(_telemetry)

func _toggle_pause() -> void:
	if Game.state != Game.State.DRIVING:
		return
	set_paused(not Game.paused)

func _resume() -> void:
	set_paused(false)

func _restart() -> void:
	if _restart_car():
		_resume()

func _restart_car() -> bool:
	if not _spawn_car():
		open_setup.emit()
		return false
	camera.reset_follow()
	_reset_run()
	_update_hud()
	return true

func _next_driver() -> void:
	Game.next_driver()
	if _restart_car():
		_resume()

func _spawn_car() -> bool:
	if is_instance_valid(car_root):
		remove_child(car_root)
		car_root.free()
		car_root = null
	if Game.car_scene == null:
		Game.setup_error = "Select a car scene before starting the stage."
		return false
	var instance := Game.car_scene.instantiate()
	var car := instance as RallyCar
	if car == null:
		Game.setup_error = "Selected car scene must use the RallyCar controller."
		instance.free()
		return false
	car.configure(Game.player_color(Game.player_index))
	if not car.place_at(_spawn_pose):
		Game.setup_error = car.configuration_error
		car.free()
		return false
	add_child(car)
	car.reset_physics_interpolation()
	car.clear_held_input()
	car_root = car
	_previous_position = car.global_position
	_telemetry = car.telemetry()
	return true

func _finish() -> void:
	_finished = true
	car_root.freeze_at_finish()
	_telemetry = car_root.telemetry()
	var previous: float = _best_times.get(Game.player_index, INF)
	_best_times[Game.player_index] = minf(previous, _elapsed)

func _out_of_world(position: Vector3) -> bool:
	if not position.is_finite():
		return true
	var height: float = stage.terrain.height_at(Vector2(position.x, position.z))
	return is_nan(height) or position.y < height - FALL_MARGIN or not car_root.linear_velocity.is_finite() or not car_root.angular_velocity.is_finite()

func _reset_run() -> void:
	_station = 0
	_progress = stage.project_distance(car_root.global_position, 0)["distance"]
	_elapsed = 0.0
	_started = false
	_finished = false
	_previous_position = car_root.global_position

func _update_hud() -> void:
	hud.update_run(Game.player_name(), Game.players.size(), _telemetry["speed"], _progress, stage.length_m, stage.pacenote(_progress), _elapsed, _finished, _best_times.get(Game.player_index, 0.0), _telemetry["surface"])

func _debug_text(data: Dictionary) -> String:
	var lines := PackedStringArray()
	lines.append("Stage %s seed %s length %s m features %s road %s m" % [track_info["generator"], track_info["seed"], track_info["length_m"], track_info["feature_count"], track_info["track_width"]])
	lines.append("Hotseat %s / %d" % [Game.player_name(), Game.players.size()])
	lines.append("speed %.2f signed %+.2f gear %s rpm %.0f steer %.2f" % [data["speed"], data["signed_speed"], data["gear"], data["rpm"], data["steer"]])
	lines.append("surface %s support %s contacts %s recoveries %s" % [data["surface"], data["support"], data["contacts"], recovery_count])
	lines.append("vel %s ang %s" % [data["velocity"], data["angular_velocity"]])
	return "\n".join(lines)
