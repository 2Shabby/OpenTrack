extends Node3D

var city: Node3D
var car: RallyCar
var spawn_pose := Transform3D.IDENTITY
var vehicle_index := 1
var overview := false
var ready_to_drive := false

@onready var camera: Camera3D = $ChaseCamera
@onready var map_camera: Camera3D = $MapCamera
@onready var label: Label = $HUD/Label

func _ready() -> void:
	var scene_path := ""
	for arg in OS.get_cmdline_user_args():
		if arg.begins_with("--city="): scene_path = arg.trim_prefix("--city=")
		if arg == "--hatchback": vehicle_index = 0
		if arg == "--overview": overview = true
	if scene_path.is_empty() or not ResourceLoader.exists(scene_path):
		label.text = "Specify a baked city scene with -- --city=<scene path>."
		return
	var packed := ResourceLoader.load(scene_path, "PackedScene") as PackedScene
	if packed == null:
		label.text = "Could not load the baked city scene."
		return
	city = packed.instantiate()
	add_child(city)
	var marker := city.get_node_or_null("Spawn") as Marker3D
	if marker == null:
		label.text = "City scene has no driving spawn."
		return
	spawn_pose = marker.global_transform
	await get_tree().physics_frame
	await get_tree().physics_frame
	_spawn()
	Game.state = Game.State.DRIVING
	ready_to_drive = true
	map_camera.current = overview
	camera.current = not overview

func _spawn() -> void:
	if is_instance_valid(car):
		remove_child(car)
		car.free()
	car = Game.VEHICLE_SCENES[vehicle_index].instantiate() as RallyCar
	car.configure(Game.player_paint_index(0))
	if not car.place_at(spawn_pose):
		push_error(car.configuration_error)
		return
	add_child(car)
	car.clear_held_input()
	camera.reset_follow()

func _physics_process(_delta: float) -> void:
	if not ready_to_drive or car == null:
		return
	if Input.is_action_just_pressed("reset_car") or car.global_position.y < -35 or not car.global_position.is_finite():
		_spawn()

func _unhandled_key_input(event: InputEvent) -> void:
	if not event is InputEventKey or not event.pressed or event.echo:
		return
	if event.physical_keycode == KEY_ESCAPE:
		get_tree().quit()
	if event.physical_keycode == KEY_M:
		overview = not overview
		map_camera.current = overview
		camera.current = not overview
	if event.physical_keycode == KEY_V and ready_to_drive:
		vehicle_index = 1 - vehicle_index
		_spawn()
	if event.physical_keycode == KEY_B and ready_to_drive:
		var marker := city.get_node_or_null("BridgeSpawn") as Marker3D
		if marker:
			spawn_pose = marker.global_transform
			_spawn()
	if event.physical_keycode == KEY_G and ready_to_drive:
		spawn_pose = (city.get_node("Spawn") as Marker3D).global_transform
		_spawn()

func _process(delta: float) -> void:
	if not ready_to_drive or car == null:
		return
	if not overview:
		camera.follow(car, car.linear_velocity, delta)
	var data := car.telemetry()
	label.text = "Bengaluru · %s · %.0f km/h · gear %s\nWASD / arrows: drive · Space: brake · R: reset · V: vehicle · M: map · B: bridge · G: ground · Esc: exit" % [Game.VEHICLE_NAMES[vehicle_index], data.speed * 3.6, data.gear]
