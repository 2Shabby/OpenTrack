extends SceneTree

const Generator := preload("res://scripts/rally_generator.gd")
const Terrain := preload("res://scripts/terrain_builder.gd")
const Settings := preload("res://scripts/terrain_settings.gd")
const Field := preload("res://scripts/terrain_field.gd")
const Shoulders := preload("res://scripts/road_shoulders.gd")
const DT := 1.0 / 120.0
var failures := 0

func _initialize() -> void:
	_run.call_deferred()

func _check(condition: bool, label: String) -> void:
	if not condition:
		failures += 1
		push_error(label)

func _run() -> void:
	var args := OS.get_cmdline_user_args()
	_check(Engine.physics_ticks_per_second == 120, "native vehicle runs at 120 Hz")
	if "--car-assets-only" in args:
		await _test_car_assets()
		print("car asset smoke: ", failures, " failures")
		quit(0 if failures == 0 else 1)
		return
	if "--shoulders-only" in args:
		await _test_shoulders()
		print("shoulder smoke: ", failures, " failures")
		quit(0 if failures == 0 else 1)
		return
	if not "--vehicle-only" in args and not "--stages-only" in args:
		_test_constraints()
		await _test_terrain_controls()
		if not "--terrain-controls" in args:
			_test_generation()
	if not "--terrain-controls" in args:
		if not "--stages-only" in args:
			await _test_car_assets()
			await _test_shoulders()
			await _test_vehicle()
			await _test_surfaces()
			await _test_air_and_collisions()
			await _test_crest()
			await _test_menus()
		if not "--vehicle-only" in args:
			await _test_generated_stages()
	print("native smoke: ", failures, " failures")
	quit(0 if failures == 0 else 1)

func _test_terrain_controls() -> void:
	for values: Array in [[20.0, 150.0, 0.10, 16.0, 500], [8.0, 1000.0, 0.01, 64.0, 5000], [0.0, 320.0, 0.06, 24.0, 500]]:
		var settings := Settings.new()
		settings.amplitude = values[0]
		settings.wavelength = values[1]
		settings.max_gradient = values[2]
		settings.blend_distance = values[3]
		var generator := Generator.new()
		var stage: Resource = generator.generate(1492, values[4], settings)
		_check(stage != null, "terrain control limits generate: %s" % generator.error)
		if stage != null:
			_check_terrain(stage)
	var game := root.get_node("Game")
	var saved: Resource = game.terrain_settings.duplicate()
	var setup: Control = load("res://scenes/ui/setup_menu.tscn").instantiate()
	root.add_child(setup)
	setup.get_node("%Amplitude").value = 8.5
	setup.get_node("%Wavelength").value = 360
	setup.get_node("%Gradient").value = 4
	setup.get_node("%Blend").value = 32
	_check(game.terrain_settings.amplitude == 8.5 and game.terrain_settings.wavelength == 360 and game.terrain_settings.max_gradient == 0.04 and game.terrain_settings.blend_distance == 32, "setup binds every terrain control and converts percent grade")
	game.terrain_settings = saved
	setup.queue_free()
	await process_frame

func _check_terrain(stage: Resource) -> float:
	var min_height := INF
	var max_height := -INF
	var previous_grade := 0.0
	var previous_length := 0.0
	for i in stage.centers.size():
		var center: Vector3 = stage.centers[i]
		min_height = minf(min_height, center.y)
		max_height = maxf(max_height, center.y)
		_check(stage.left_edges[i].y == stage.right_edges[i].y, "road cross-sections have no banking")
		_check(absf(stage.left_edges[i].distance_to(stage.right_edges[i]) - stage.ROAD_WIDTH) < 0.001, "draping preserves road width")
		if i == 0:
			continue
		var delta: Vector3 = center - stage.centers[i - 1]
		var rise := delta.y
		delta.y = 0
		var length := delta.length()
		var grade := rise / length
		_check(absf(grade) <= stage.terrain_settings.max_gradient + 0.00002, "centerline obeys grade limit")
		for edges: PackedVector3Array in [stage.left_edges, stage.right_edges]:
			var edge := edges[i] - edges[i - 1]
			edge.y = 0
			_check(absf(rise) / edge.length() <= stage.terrain_settings.max_gradient + 0.00002, "inner and outer road edges obey grade limit")
		if i > 1:
			_check(absf(grade - previous_grade) <= Terrain.MAX_GRADE_CHANGE * (length + previous_length) * 0.5 + 0.00002, "road crests obey grade-change limit")
		previous_grade = grade
		previous_length = length
		for edges: PackedVector3Array in [stage.left_edges, stage.right_edges]:
			for t: float in [0, 0.5, 1]:
				var point := edges[i - 1].lerp(edges[i], t)
				var gap: float = point.y - stage.terrain.height_at(Vector2(point.x, point.z))
				_check(gap >= Field.ROAD_CLEARANCE - 0.0001 and gap < 0.75, "buried road-edge clearance has lower and upper bounds")
		for triangle: Array in [[stage.left_edges[i - 1], stage.right_edges[i - 1], stage.left_edges[i]], [stage.right_edges[i - 1], stage.right_edges[i], stage.left_edges[i]]]:
			for weights: Vector3 in [Vector3.ONE / 3, Vector3(0.8, 0.1, 0.1), Vector3(0.1, 0.8, 0.1), Vector3(0.1, 0.1, 0.8)]:
				var point: Vector3 = triangle[0] * weights.x + triangle[1] * weights.y + triangle[2] * weights.z
				_check(stage.terrain.height_at(Vector2(point.x, point.z)) <= point.y - Field.ROAD_CLEARANCE + 0.0001, "stamped terrain stays below actual road triangles")
	_check(max_height - min_height > 0.1 if stage.terrain_settings.amplitude > 0 else max_height == min_height, "road relief follows its amplitude control")
	var pieces: Array = stage.pieces()
	for i in range(1, pieces.size()):
		_check(pieces[i - 1]["vertices"][-2] == pieces[i]["vertices"][0] and pieces[i - 1]["vertices"][-1] == pieces[i]["vertices"][1], "road pieces share exact boundary vertices")
		_check(pieces[i - 1]["normals"][-2] == pieces[i]["normals"][0] and pieces[i - 1]["normals"][-1] == pieces[i]["normals"][1], "road pieces share exact boundary normals")
	_check(Terrain.new()._terrain_is_gentle(stage.terrain), "terrain has no cut-and-fill cliffs")
	return _check_shoulders(stage)

func _check_shoulders(stage: Resource) -> float:
	var meshes: Array[ArrayMesh] = Shoulders.new().build(stage)
	var max_grade := 0.0
	var max_seam_error := 0.0
	var invalid_faces := 0
	var side_ends: Array[PackedVector3Array] = []
	for side in meshes.size():
		var arrays := meshes[side].surface_get_arrays(0)
		var vertices: PackedVector3Array = arrays[Mesh.ARRAY_VERTEX]
		var indices: PackedInt32Array = arrays[Mesh.ARRAY_INDEX]
		var joints := {}
		for i in range(0, vertices.size(), 2):
			joints[vertices[i]] = true
			if i == 0:
				continue
			for t: float in [0, 0.25, 0.5, 0.75, 1]:
				var point := vertices[i - 1].lerp(vertices[i + 1], t)
				var height: float = stage.terrain.height_at(Vector2(point.x, point.z))
				max_seam_error = maxf(max_seam_error, absf(point.y - height))
		if side < 2:
			var edges: PackedVector3Array = stage.left_edges if side == 0 else stage.right_edges
			for edge in edges:
				_check(joints.has(edge), "grass shares exact road cross-section boundary vertices")
			side_ends.append(PackedVector3Array([vertices[0], vertices[1], vertices[-2], vertices[-1]]))
		else:
			var end := (side - 2) * 2
			_check(vertices[0] == side_ends[0][end] and vertices[1] == side_ends[0][end + 1] and vertices[-2] == side_ends[1][end] and vertices[-1] == side_ends[1][end + 1], "start and finish shoulders close both side strips with exact corner vertices")
		for i in range(0, indices.size(), 3):
			var a := vertices[indices[i]]
			var b := vertices[indices[i + 1]]
			var c := vertices[indices[i + 2]]
			var normal := (b - a).cross(c - a)
			if normal.y >= 0:
				invalid_faces += 1
			elif absf(normal.y) > 0.0001:
				max_grade = maxf(max_grade, Vector2(normal.x, normal.z).length() / absf(normal.y))
	_check(invalid_faces == 0, "shoulders have valid clockwise triangles")
	_check(max_seam_error < 0.001, "outer shoulder edges conform to terrain cell edges and diagonals")
	_check(max_grade <= Terrain.MAX_TERRAIN_GRADIENT + 0.005, "grass verge triangles have no steep cut-and-fill cliffs: %.3f" % max_grade)
	return max_grade

func _test_shoulders() -> void:
	var stage: Resource
	for seed_value: int in [0, 1, 1492, 1592598566]:
		stage = Generator.new().generate(seed_value, 1200, Settings.new())
		_check(stage != null, "shoulder regression seed generates: %s" % seed_value)
		if stage != null:
			print("shoulder seed ", seed_value, " maximum verge grade ", _check_terrain(stage))
	if stage == null:
		return
	var track := _track(stage)
	await _frames(3)
	# The old 4.45 m depression was here, in the default first tight corner.
	for edges: PackedVector3Array in [stage.left_edges, stage.right_edges]:
		var edge := edges[49]
		var outward: Vector3 = (edge - stage.centers[49]).normalized()
		for offset: float in [-0.02, 0.02, 0.25, 1.0, 2.0, 2.02, 3.0]:
			var point := edge + outward * offset
			var hit := _support_hit(track, point)
			_check(not hit.is_empty(), "native support spans former shoulder gap")
			if hit.is_empty():
				continue
			var height: float = hit["position"].y
			if absf(offset) <= 0.02:
				_check(absf(height - edge.y) < 0.015, "native road/grass boundary is continuous")
			if offset > 0:
				_check(hit["collider"].get_meta("surface") == TrackGeometry.GRASS, "connected shoulder uses native Grass contacts")
			if offset >= Shoulders.WIDTH:
				var field_height: float = stage.terrain.height_at(Vector2(point.x, point.z))
				_check(absf(height - field_height) < 0.002, "native outer shoulder joint matches terrain collision")
		for direction: float in [1, -1]:
			var forward := outward * direction
			var start := edge - forward * 3
			var hit := _support_hit(track, start)
			start.y = hit["position"].y
			var pose := Transform3D(Basis(Vector3.UP.cross(forward), Vector3.UP, forward), start)
			var car := _new_car(pose)
			await _frames(120)
			car.linear_velocity = forward * 5
			for wheel: Wheel in car.wheel_array:
				wheel.spin = 5 / wheel.tire_radius
			car.throttle_input = 0.4
			var air_ticks := 0
			var crossed := false
			for _i in 240:
				await physics_frame
				air_ticks += 1 if car.airborne() else 0
				if (car.global_position - edge).dot(forward) > 1:
					crossed = true
					break
			_check(crossed and air_ticks < 6 and car.global_transform.is_finite(), "native car crosses default verge in direction %s without falling through" % direction)
			print("native verge crossing ", direction, " crossed ", crossed, " air ticks ", air_ticks)
			await _dispose([car])
	await _dispose([track])

func _support_hit(track: Node3D, point: Vector3) -> Dictionary:
	var query := PhysicsRayQueryParameters3D.create(point + Vector3.UP * 10, point - Vector3.UP * 10, TrackGeometry.SUPPORT_MASK)
	return track.get_world_3d().direct_space_state.intersect_ray(query)

func _test_constraints() -> void:
	var settings := Settings.new()
	settings.amplitude = NAN
	_check(Generator.new().generate(1, 500, settings) == null, "nonfinite terrain controls are rejected")
	var stage := Generator.Stage.new()
	# A synthetic hairpin has parallel legs 14 m apart, 400 m apart by road.
	for i in 101:
		stage.centers.append(Vector3(0, 0, i * 2))
		stage.headings.append(0)
		stage.distances.append(i * 2)
	for i in range(1, 12):
		var angle := PI * i / 11
		stage.centers.append(Vector3(7 - 7 * cos(angle), 0, 200 + 7 * sin(angle)))
		stage.headings.append(angle)
		stage.distances.append(stage.distances[-1] + stage.centers[-1].distance_to(stage.centers[-2]))
	for i in range(1, 101):
		stage.centers.append(Vector3(14, 0, 200 - i * 2))
		stage.headings.append(PI)
		stage.distances.append(stage.distances[-1] + 2)
	var builder := Terrain.new()
	builder._config = Settings.new()
	builder._find_neighbors(stage)
	var heights := PackedFloat64Array()
	for i in stage.centers.size():
		heights.append(8.0 * i / (stage.centers.size() - 1))
	builder._constrain(stage, heights)
	_check(absf(heights[-1] - heights[0]) <= 0.40001, "nearby hairpin legs cannot acquire incompatible heights")
	_check(not builder._neighbors.is_empty(), "hairpin solver checks nonadjacent legs")
	for pair: Vector3 in builder._neighbors:
		_check(absf(heights[int(pair.y)] - heights[int(pair.x)]) <= pair.z + 0.00001, "all nearby-road height constraints hold")
	stage = Generator.Stage.new()
	for i in 4:
		stage.centers.append(Vector3(0, 0, i * 2))
		stage.headings.append(0)
		stage.distances.append(i * 2)
	builder._neighbors = PackedVector3Array([Vector3(2, 3, 0)])
	heights = PackedFloat64Array([1.0, 1.002, 1.004, 1.004])
	builder._constrain(stage, heights)
	_check(heights[-1] - heights[0] > 0.003, "already satisfied equal-height constraints preserve other road relief")

func _test_generation() -> void:
	var generator := Generator.new()
	_check(generator.generate(1, 499, Settings.new()) == null and generator.generate(1, 5001, Settings.new()) == null, "stage bounds reject invalid length")
	var linked := 0
	var long_straights := 0
	var surfaces := {}
	var corner_lengths := {}
	var started := Time.get_ticks_msec()
	for seed_value in [0, 1, -1, 1492, 1592598566, 9223372036854775807, -9223372036854775808]:
		for length in [500, 1200, 5000]:
			var stage = generator.generate(seed_value, length, Settings.new())
			_check(stage != null, "stage generates: %s / %s" % [seed_value, length])
			if stage == null:
				continue
			var replay = generator.generate(seed_value, length, Settings.new())
			_check(replay != null and replay.centers == stage.centers and replay.features == stage.features, "recipe repeats from seed and length")
			_check(stage.requested_length_m == length and stage.length_m >= length - 0.01 and stage.length_m <= length * sqrt(1 + pow(Settings.new().max_gradient, 2)) + 0.01, "stage measures actual draped length from requested horizontal length")
			_check(replay.terrain.heights == stage.terrain.heights, "terrain repeats from seed and controls")
			_check_terrain(stage)
			_check(stage.features.front()["kind"] == "straight" and stage.features.back()["kind"] == "straight", "start and finish have straights")
			var previous := ""
			var end := 0.0
			for feature: Dictionary in stage.features:
				_check(absf(feature["start_m"] - end) < 0.001 and feature["length_m"] > 0, "features have continuous positive distance")
				end = feature["end_m"]
				_check(feature["surface"] in [0, 1], "only dirt and asphalt")
				surfaces[feature["surface"]] = true
				if feature["kind"] == "corner":
					_check(feature["grade"] >= 1 and feature["grade"] <= 6, "corner grade is valid")
					var peak_curvature := 0.0
					for i in range(feature["first_station"] + 1, feature["last_station"]):
						var entry: Vector3 = stage.centers[i] - stage.centers[i - 1]
						var exit: Vector3 = stage.centers[i + 1] - stage.centers[i]
						entry.y = 0
						exit.y = 0
						peak_curvature = maxf(peak_curvature, entry.angle_to(exit) / ((entry.length() + exit.length()) * 0.5))
					var expected_radius: float = Generator.RADII[feature["grade"] - 1]
					_check(absf(1.0 / peak_curvature - expected_radius) < expected_radius * 0.03, "grade agrees with curvature of actual road centers")
					corner_lengths[roundi(feature["length_m"])] = true
					if previous == "corner":
						linked += 1
				else:
					_check(feature["length_m"] >= 39.99, "straights have meaningful length")
					if feature["length_m"] >= 150:
						long_straights += 1
				previous = feature["kind"]
			var occupied := {}
			for i in stage.centers.size():
				var point: Vector3 = stage.centers[i]
				_check(point.is_finite(), "finite draped road")
				if i > 0:
					_check(point.distance_to(stage.centers[i - 1]) <= 2.011, "dense road stations")
				var cell := Vector2i(floori(point.x / 10), floori(point.z / 10))
				for x in range(cell.x - 2, cell.x + 3):
					for y in range(cell.y - 2, cell.y + 3):
						for j: int in occupied.get(Vector2i(x, y), []):
							if stage.distances[i] - stage.distances[j] >= 24.05:
								_check(point.distance_to(stage.centers[j]) >= Generator.CLEARANCE, "road clearance, including within a corner")
				if not occupied.has(cell):
					occupied[cell] = []
				occupied[cell].append(i)
			var projected: Dictionary = stage.project_distance(stage.centers[20], 0)
			_check(absf(projected["distance"] - stage.distances[20]) < 0.001, "pacenote progress projects onto road distance")
			var corner: Dictionary = stage.features[1]
			_check(stage.pacenote(corner["start_m"]).contains("%s %d" % [corner["direction"], corner["grade"]]), "pacenote describes actual corner")
	_check(linked > 0 and long_straights > 0 and corner_lengths.size() > 5 and surfaces.size() == 2, "recipe mixes linked corners, varied lengths, long straights and both surfaces")
	print("rally generation: 42 recipes in ", Time.get_ticks_msec() - started, " ms; linked corners ", linked, ", long straights ", long_straights)
	var baked = generator.generate(1592598566, 1200, Settings.new())
	for piece: Dictionary in baked.pieces():
		var vertices: PackedVector3Array = piece["vertices"]
		var indices: PackedInt32Array = piece["indices"]
		for i in range(0, indices.size(), 3):
			var a := vertices[indices[i]]
			var b := vertices[indices[i + 1]]
			var c := vertices[indices[i + 2]]
			_check((b - a).cross(c - a).y < 0, "road triangles use Godot clockwise front faces from above")
	_check(ResourceSaver.save(baked, "/tmp/opentrack-rally-stage.res") == OK, "stage saves as a native Godot resource")
	var loaded = ResourceLoader.load("/tmp/opentrack-rally-stage.res", "", ResourceLoader.CACHE_MODE_IGNORE)
	_check(loaded != null and loaded.centers == baked.centers and loaded.features == baked.features and loaded.terrain.heights == baked.terrain.heights and loaded.terrain_settings.max_gradient == baked.terrain_settings.max_gradient, "saved stage preserves geometry and pacenote recipe")

# These checks run the production scene through its own physics callbacks.
# Test controls use GEVP's public input fields; there is no second solver.
func _test_car_assets() -> void:
	var game := root.get_node("Game")
	var colors := {}
	for i in game.MAX_PLAYERS:
		colors[game.player_color(i)] = true
	_check(colors.size() == game.MAX_PLAYERS, "all hotseat drivers have stable distinct paint colors")
	var first: RallyCar = game.car_scene.instantiate()
	var second: RallyCar = game.car_scene.instantiate()
	first.configure(game.player_color(0))
	second.configure(game.player_color(1))
	_check(first.prepare() and second.prepare(), "voxel car satisfies the shared rig contract before scene entry")
	_check(first.visual._paint.size() > 0 and first.visual._paint[0] != second.visual._paint[0], "paint materials are isolated per car instance")
	_check(first.visual._paint[0].albedo_color == game.player_color(0) and second.visual._paint[0].albedo_color == game.player_color(1), "paint uses player color without altering other materials")
	var glass: Material = first.visual.get_node("Model/CarModel/Body").get_active_material(0)
	first.visual.set_lights(1, true)
	_check(first.visual._tail[0].emission_energy_multiplier > 0 and first.visual._brake[0].emission_energy_multiplier == 3 and first.visual._reverse[0].emission_energy_multiplier == 1.5, "tail, brake and reverse lamps have separate active states")
	_check(second.visual._brake[0].emission_energy_multiplier == 0 and second.visual._reverse[0].emission_energy_multiplier == 0, "lamp materials are isolated per car")
	first.configure(game.player_color(2))
	_check(first.visual._brake[0].emission_energy_multiplier == 3 and first.visual.get_node("Model/CarModel/Body").get_active_material(0) == glass, "paint changes preserve lamp state and glazing")
	first.free()
	second.free()
	var invalid: RallyCar = game.car_scene.instantiate()
	var visual: CarVisual = invalid.get_node("Visual")
	visual.wheels[0] = visual.wheels[1]
	_check(not invalid.prepare() and not invalid.configuration_error.is_empty(), "invalid wheel bindings are rejected before GEVP initialization")
	invalid.free()
	var track := _track(_fixture())
	for split: float in [0.0, 0.5, 1.0]:
		var car: RallyCar = game.car_scene.instantiate()
		car.accept_input = false
		car.front_torque_split = split
		_check(car.place_at(_fixture_pose()), "drivetrain fixture prepares")
		root.add_child(car)
		await _frames(180)
		_check(car.front_left_wheel.is_driven == (split > 0) and car.front_right_wheel.is_driven == (split > 0) and car.rear_left_wheel.is_driven == (split < 1) and car.rear_right_wheel.is_driven == (split < 1), "GEVP selects correct driven axles at split %s" % split)
		_check(car.visual._brake[0].emission_energy_multiplier == 0, "spawn parking handbrake does not illuminate service brake lamps")
		car.throttle_input = 1
		await _frames(360)
		_check(car.telemetry()["signed_speed"] > 8 and car.contact_count() == 4, "AWD/FWD/RWD fixture drives under native tire forces")
		_check(absf(car.front_left_wheel.wheel_node.rotation.x) > 0.1 and car.front_left_wheel.wheel_node.position.y < 0, "wheel visuals follow native spin and suspension travel")
		var rear_toe := car.rear_left_wheel.rotation.y
		car.steering_input = 0.3
		await _frames(12)
		_check(absf(car.front_left_wheel.rotation.y) > 0.01 and is_equal_approx(car.rear_left_wheel.rotation.y, rear_toe), "front wheel visuals steer while rear wheels retain their authored toe")
		car.steering_input = 0
		car.throttle_input = 0
		car.brake_input = 1
		await _frames(30)
		_check(car.visual._brake[0].emission_energy_multiplier > 2, "actual GEVP service braking lights brake lamps")
		car.current_gear = -1
		await _frames(2)
		_check(car.visual._reverse[0].emission_energy_multiplier == 1.5, "actual reverse gear lights white lamps")
		var brake_glow := car.visual._brake[0].emission_energy_multiplier
		car.freeze_at_finish()
		await _frames(2)
		_check(car.visual._brake[0].emission_energy_multiplier == brake_glow and car.visual._reverse[0].emission_energy_multiplier == 1.5, "finish preserves lamp state")
		print("car drivetrain split ", split, " verified")
		await _dispose([car])
	await _dispose([track])

func _fixture_pose() -> Transform3D:
	return Transform3D(Basis.IDENTITY, Vector3(0, 0, -110))

func _frames(count: int) -> void:
	for _i in count:
		await physics_frame

func _new_car(pose: Transform3D) -> RallyCar:
	var car: RallyCar = root.get_node("Game").car_scene.instantiate()
	car.accept_input = false
	car.place_at(pose)
	root.add_child(car)
	return car

func _dispose(nodes: Array) -> void:
	for node: Node in nodes:
		node.queue_free()
	await _frames(2)

func _fixture(surface: int = 0) -> Resource:
	var stage := Generator.Stage.new()
	for i in 151:
		stage.centers.append(Vector3(0, 0, -150 + i * 2))
		stage.headings.append(0)
		stage.distances.append(i * 2)
	stage.features.append({"kind": "straight", "surface": surface, "first_station": 0, "last_station": 150})
	var settings := Settings.new()
	settings.amplitude = 0
	_check(Terrain.new().apply(stage, settings), "flat fixture uses terrain pipeline")
	return stage

func _track(stage: Resource) -> TrackGeometry:
	var track := TrackGeometry.new()
	root.add_child(track)
	track.build(stage)
	return track

func _test_vehicle() -> void:
	var stage := _fixture()
	var track := _track(stage)
	var car := _new_car(stage.road_pose(60))
	_check(car.basis.determinant() > 0.999 and car.forward_vector().dot(Vector3.BACK) > 0.999, "spawn is a proper rotation pointing along road")
	_check(car.front_left_wheel.position.x < 0 and car.front_right_wheel.position.x > 0 and car.front_left_wheel.position.z < car.rear_left_wheel.position.z, "physical left/right and front/rear axle ordering")
	_check(is_equal_approx(car.front_tire_radius, 0.325) and is_equal_approx(car.front_tire_width, 200), "tire dimensions originate in the voxel asset")
	_check(car.collision_layer == 4 and car.collision_mask == 3 and car.continuous_cd, "chassis collides with road and terrain")
	await _frames(360)
	_check(car.contact_count() == 4 and absf(car.linear_velocity.y) < 0.05, "four suspension rays settle on road")
	var collider: CollisionShape3D = car.get_node("CollisionShape3D")
	var bottom: float = car.global_position.y + collider.position.y - collider.shape.size.y * 0.5
	_check(bottom > 0.01, "settled chassis clears road")
	for wheel: Wheel in car.wheel_array:
		_check(wheel.spring_current_length > 0 and wheel.spring_current_length < wheel.spring_length and wheel.wheel_node != null, "independent wheel suspension and visual")
	var start := car.global_position
	car.throttle_input = 1
	await _frames(480)
	var speed := car.linear_velocity.length()
	_check(speed > 10 and car.global_position.z > start.z + 20 and car.current_gear > 0, "AWD automatic accelerates forward")
	car.throttle_input = 0
	car.brake_input = 1
	await _frames(240)
	var braked_speed := car.linear_velocity.length()
	_check(braked_speed < speed * 0.5, "service brake sheds at least half road speed within two seconds")
	car.accept_input = true
	Input.action_press("throttle_negative")
	await _frames(600)
	_check(car.current_gear == -1 and car.telemetry()["signed_speed"] < -1, "S brakes then drives reverse through automatic gearbox")
	Input.action_release("throttle_negative")
	print("vehicle: radius ", car.front_tire_radius, " width ", car.front_tire_width, " mm; acceleration speed ", speed, " braked speed ", braked_speed, " m/s")
	await _dispose([car])
	car = _new_car(stage.road_pose(80))
	await _frames(240)
	car.throttle_input = 0.7
	await _frames(240)
	car.steering_input = 0.3
	await _frames(90)
	_check(car.forward_vector().x > 0.08, "A/positive steering turns left while travelling forward")
	car.throttle_input = 0
	car.steering_input = 0
	car.handbrake_input = 1
	car.clutch_input = 1
	await _frames(90)
	_check(absf(car.rear_left_wheel.spin) < absf(car.front_left_wheel.spin) + 1, "handbrake brakes independent rear wheels")
	await _dispose([car, track])

func _test_surfaces() -> void:
	var stage := _fixture()
	var track := _track(stage)
	_check(track.has_node("Ground") and not track.has_node("LeftRail"), "green terrain has no guardrails")
	for body: Node in track.get_children():
		if body is StaticBody3D:
			_check(body.get_groups() == [&"Road"], "road has one recognized surface group")
	var speeds := []
	for x: float in [0.0, 20.0]:
		var pose: Transform3D = stage.road_pose(40)
		pose.origin.x = x
		var car := _new_car(pose)
		await _frames(240)
		car.throttle_input = 1
		await _frames(720)
		if x != 0:
			await _frames(1680)
		speeds.append(car.linear_velocity.length())
		_check(car.contact_count() == 4 and car.linear_velocity.is_finite(), "all wheels drive on surface at x=%s" % x)
		_check(car.telemetry()["surface"] == ("asphalt" if x == 0 else "grass"), "surface telemetry comes from actual wheel collider")
		await _dispose([car])
	_check(speeds[0] > 15 and speeds[1] > 0.5 and speeds[1] < speeds[0] * 0.4, "grass is slow and remains driveable without a speed clamp")
	print("surface speeds: road after 6s ", speeds[0], " grass after 20s ", speeds[1], " m/s")
	var pose: Transform3D = stage.road_pose(60)
	pose.origin.x = 6
	var mixed := _new_car(pose)
	await _frames(240)
	var surfaces: PackedStringArray = mixed.telemetry()["wheels"]
	_check(surfaces.count("Road") == 2 and surfaces.count("Grass") == 2, "road-edge wheels independently sample mixed surfaces")
	await _dispose([mixed])
	pose.origin.x = 20
	var entry := _new_car(pose)
	await _frames(240)
	entry.linear_velocity = Vector3(0, 0, 25)
	for wheel: Wheel in entry.wheel_array:
		wheel.spin = 25 / wheel.tire_radius
	await _frames(12)
	_check(entry.linear_velocity.length() > 20 and entry.linear_velocity.length() < 25, "fast grass entry slows progressively instead of clamping speed")
	await _dispose([entry])
	pose.origin = Vector3(20, 0, -30)
	pose.basis = Basis(Vector3.UP, -PI / 2)
	var rejoin := _new_car(pose)
	await _frames(240)
	rejoin.throttle_input = 1
	var reached := false
	for _i in 2400:
		await physics_frame
		if rejoin.telemetry()["surface"] == "asphalt" and absf(rejoin.global_position.x) < 5:
			reached = true
			break
	_check(reached and rejoin.linear_velocity.is_finite(), "native vehicle can drive back across grass onto road")
	await _dispose([rejoin, track])
	var dirt_track := _track(_fixture(1))
	var dirt := _new_car(stage.road_pose(60))
	await _frames(240)
	dirt.throttle_input = 1
	await _frames(480)
	_check(dirt.telemetry()["surface"] == "dirt" and dirt.linear_velocity.length() > 5, "dirt collider uses library tire parameters")
	await _dispose([dirt, dirt_track])

func _test_air_and_collisions() -> void:
	var stage := _fixture()
	var track := _track(stage)
	var pose: Transform3D = stage.road_pose(60)
	pose.origin.y += 3
	var car := _new_car(pose)
	await _frames(8)
	_check(car.airborne() and car.linear_velocity.y < -0.3, "native gravity drops the chassis with no support")
	await _frames(480)
	_check(car.contact_count() == 4 and absf(car.linear_velocity.y) < 0.1, "hard drop lands and suspension settles")
	car.apply_central_impulse(Vector3(0, 6, 8) * car.mass)
	car.angular_velocity = Vector3(0.25, 0.3, 0.2)
	var airborne_frames := 0
	var peak := car.global_position.y
	var horizontal := car.global_position.z
	for _i in 240:
		await physics_frame
		if car.airborne():
			airborne_frames += 1
		peak = maxf(peak, car.global_position.y)
		_check(car.global_transform.is_finite() and car.linear_velocity.is_finite(), "finite jump and angled landing")
	_check(airborne_frames > 60 and peak > 1 and car.global_position.z > horizontal + 5, "jump unloads wheels while retaining horizontal momentum")
	await _frames(360)
	_check(car.contact_count() >= 3, "angled landing regains suspension contact")
	await _dispose([car])
	pose.origin.y = 8
	var rolled := _new_car(pose)
	rolled.global_basis = rolled.global_basis * Basis(Vector3.FORWARD, PI * 0.65)
	rolled.angular_velocity = Vector3(0, 0.7, 0)
	await _frames(12)
	_check(rolled.airborne() and rolled.global_basis.y.dot(Vector3.UP) < 0 and absf(rolled.angular_velocity.y) > 0.5, "airborne rollover is not forcibly upright or yaw-stabilized")
	var camera := Camera3D.new()
	camera.set_script(preload("res://scripts/chase_camera.gd"))
	root.add_child(camera)
	camera.follow(rolled, rolled.linear_velocity, DT)
	_check(camera.global_transform.is_finite() and camera.global_basis.y.dot(Vector3.UP) > 0.5, "rollover camera keeps gravity-up")
	await _frames(600)
	_check(rolled.global_position.y > -1 and rolled.global_position.y < 2 and rolled.global_transform.is_finite(), "chassis collision catches a roof/side landing")
	print("jump: ", airborne_frames, " airborne ticks; peak ", peak, " m")
	await _dispose([rolled, camera, track])

func _test_crest() -> void:
	# Analytic road: <10% grade and <0.006/m grade change, matching production limits.
	var stage := _fixture()
	for i in stage.centers.size():
		stage.centers[i].y = 1.8 * cos(stage.centers[i].z * PI / 60)
	stage.rebuild_road()
	var builder := Terrain.new()
	builder._config = stage.terrain_settings
	builder._index_segments(stage)
	stage.terrain = builder._heightfield(stage)
	var track := _track(stage)
	var car := _new_car(stage.road_pose(120))
	await _frames(240)
	car.linear_velocity = car.forward_vector() * 55
	for wheel: Wheel in car.wheel_array:
		wheel.spin = 55 / wheel.tire_radius
	var air_ticks := 0
	var partial_ticks := 0
	var landed := false
	for _i in 360:
		await physics_frame
		air_ticks += 1 if car.airborne() else 0
		partial_ticks += 1 if car.contact_count() in [1, 2] else 0
		landed = landed or (air_ticks > 5 and car.contact_count() >= 3)
		_check(car.linear_velocity.is_finite() and car.global_transform.is_finite(), "finite crest motion")
	_check(air_ticks > 5 and partial_ticks > 0 and landed and car.global_position.z > 50, "road crest causes real takeoff, partial wheel unloading and landing")
	print("crest: air ticks ", air_ticks, " partial ticks ", partial_ticks)
	await _dispose([car, track])

func _test_generated_stages() -> void:
	for length: int in [500, 1200, 5000]:
		var stage: Resource = Generator.new().generate(1592598566, length, Settings.new())
		_check(stage != null, "native traversal stage generates")
		if stage == null:
			continue
		var track := _track(stage)
		var car := _new_car(stage.spawn_pose())
		await _frames(240)
		for grid: Vector2 in [Vector2(32.25, 32.5), Vector2(32.75, 32.6), Vector2(32, 33.25)]:
			var point: Vector2 = stage.terrain.origin + grid * Field.SPACING
			var height: float = stage.terrain.height_at(point)
			var query := PhysicsRayQueryParameters3D.create(Vector3(point.x, height + 2, point.y), Vector3(point.x, height - 2, point.y), TrackGeometry.TERRAIN_LAYER)
			var hit := car.get_world_3d().direct_space_state.intersect_ray(query)
			_check(not hit.is_empty() and absf(hit["position"].y - height) < 0.0001, "terrain collision matches heightfield at chunk seams")
		var station := 0
		var finished := false
		var previous := car.global_position
		var max_offset := 0.0
		var air_ticks := 0
		var partial_ticks := 0
		for tick in 120 * length:
			await physics_frame
			var projection: Dictionary = stage.project_distance(car.global_position, station)
			station = projection["station"]
			var distance: float = projection["distance"]
			max_offset = maxf(max_offset, sqrt(projection["offset_squared"]))
			if not car.global_transform.is_finite() or not car.linear_velocity.is_finite():
				break
			var speed := car.linear_velocity.length()
			var target: Transform3D = stage.road_pose(minf(distance + 5 + speed * 0.3, stage.length_m))
			var direction := (target.origin - car.global_position).slide(Vector3.UP).normalized()
			var local := car.global_basis.inverse() * direction
			car.steering_input = clampf(atan2(-local.x, -local.z) * 2.2, -1, 1)
			var target_speed := 12.0
			for feature: Dictionary in stage.features:
				if feature["kind"] == "corner" and feature["end_m"] > distance and feature["start_m"] < distance + 35:
					target_speed = minf(target_speed, sqrt(Generator.RADII[feature["grade"] - 1] * 2.8))
			car.throttle_input = clampf((target_speed - speed) * 0.6, 0, 1)
			car.brake_input = clampf((speed - target_speed) * 0.4, 0, 0.7)
			air_ticks += 1 if car.airborne() else 0
			partial_ticks += 1 if car.contact_count() in [1, 2] else 0
			if stage.crossed_finish(previous, car.global_position):
				finished = true
				break
			previous = car.global_position
			if tick % 6000 == 5999:
				print("traversal ", length, " m: ", roundi(distance), " m, offset ", sqrt(projection["offset_squared"]), " speed ", speed)
			if sqrt(projection["offset_squared"]) > 20:
				break
		_check(finished and car.linear_velocity.is_finite() and max_offset < 6, "native car completes %s m generated stage without reset" % length)
		print("stage ", length, " m: finished ", finished, " max offset ", max_offset, " air ticks ", air_ticks, " partial ticks ", partial_ticks)
		await _dispose([car, track])

func _test_menus() -> void:
	var game := root.get_node("Game")
	var saved_car: PackedScene = game.car_scene
	# A runtime fixture of this same model tests replacement without another asset.
	var template: RallyCar = saved_car.instantiate()
	template.front_torque_split = 0.0
	var replacement := PackedScene.new()
	_check(replacement.pack(template) == OK, "replacement car configuration packs as a native scene")
	template.free()
	game.car_scene = replacement
	var app: Node = load("res://scenes/main.tscn").instantiate()
	root.add_child(app)
	app.content.get_node("%Start").pressed.emit()
	await process_frame
	_check(app.content.get_node("%Players") is SpinBox, "setup uses native numeric controls")
	app.content.get_node("%Seed").text = "9223372036854775808"
	app.content._validate_seed("9223372036854775808")
	_check(app.content.get_node("%Start").disabled, "overflowing seed rejected")
	app.content.get_node("%Seed").text = "1592598566"
	app.content.get_node("%Start").pressed.emit()
	var race: Node = app.content
	_check(race.car_root is RigidBody3D and race.stage.requested_length_m == 1200 and not race.hud.get_node("%Pacenote").text.is_empty(), "setup starts native vehicle and rally HUD")
	_check(race.car_root.front_torque_split == 0.0, "world spawns the selected car scene rather than a hardcoded model")
	await _frames(240)
	var parked_position: Vector3 = race.car_root.global_position
	await _frames(240)
	_check(race.car_root.global_position.distance_to(parked_position) < 0.05 and not race._started, "native handbrake holds slope before first timed drive input")
	Input.action_press("throttle_positive")
	await _frames(120)
	_check(race._started and race._elapsed > 0.5, "first accepted drive input starts timer")
	race.car_root.apply_central_impulse(Vector3.UP * race.car_root.mass * 6)
	await _frames(12)
	_check(race.car_root.airborne(), "run survives takeoff without recovery")
	race.set_paused(true)
	var position: Transform3D = race.car_root.global_transform
	var velocity: Vector3 = race.car_root.linear_velocity
	var rotation_speed: Vector3 = race.car_root.angular_velocity
	var elapsed: float = race._elapsed
	for _i in 8:
		await process_frame
	_check(race.car_root.global_transform.is_equal_approx(position) and race.car_root.linear_velocity.is_equal_approx(velocity) and race.car_root.angular_velocity.is_equal_approx(rotation_speed) and race._elapsed == elapsed, "pause freezes airborne pose, velocities and timer")
	race.pause_menu.get_node("%Resume").pressed.emit()
	await _frames(12)
	_check(not paused and not game.paused and race._elapsed > elapsed and race.car_root.global_position.distance_to(position.origin) > 0.05, "resume continues airborne trajectory and timing")
	race.pause_menu.get_node("%Restart").pressed.emit()
	await _frames(30)
	_check(not race._started and race._elapsed == 0 and race.car_root.throttle_input == 0, "retry suppresses held drive input and timer")
	Input.action_release("throttle_positive")
	await _frames(2)
	Input.action_press("throttle_positive")
	await _frames(12)
	_check(race._started, "release and repress starts next attempt")
	Input.action_release("throttle_positive")
	var old: int = race.car_root.get_instance_id()
	Input.action_press("reset_car")
	await _frames(2)
	Input.action_release("reset_car")
	_check(race.car_root.get_instance_id() != old and not race._started, "R replaces an active car and resets attempt")
	var finish: Transform3D = race.stage.road_pose(race.stage.length_m)
	for height: float in [0.5, 8.0]:
		_check(race.stage.crossed_finish(finish * Vector3(0, height, -5), finish * Vector3(0, height, 5)), "swept grounded/airborne finish")
	_check(not race.stage.crossed_finish(finish * Vector3(0, 1, 5), finish * Vector3(0, 1, -5)), "finish rejects reverse crossing")
	_check(not race.stage.crossed_finish(finish * Vector3(7, 1, -5), finish * Vector3(7, 1, 5)), "finish rejects crossing outside road")
	race._started = true
	race._elapsed = 10
	var approach := finish
	approach.origin = finish * Vector3(0, 8, -3)
	race.car_root.place_at(approach)
	race._previous_position = race.car_root.global_position
	race.car_root.linear_velocity = finish.basis.z * 30
	await _frames(30)
	_check(race._finished and race._best_times.has(0) and race.car_root.freeze, "airborne finish records time and freezes body")
	var finished_pose: Transform3D = race.car_root.global_transform
	var finished_wheel: Transform3D = race.car_root.front_left_wheel.wheel_node.transform
	var best: float = race._best_times[0]
	await _frames(12)
	_check(race._best_times[0] == best and race.car_root.global_transform.is_equal_approx(finished_pose) and race.car_root.front_left_wheel.wheel_node.transform.is_equal_approx(finished_wheel), "finish records once and freezes chassis/wheel visuals")
	var stage_id: int = race.stage.get_instance_id()
	old = race.car_root.get_instance_id()
	race.pause_menu.get_node("%NextDriver").pressed.emit()
	_check(game.player_index == 1 and race._elapsed == 0 and not race._finished and race._best_times.has(0) and race.stage.get_instance_id() == stage_id, "hotseat preserves stage and driver bests")
	_check(race.car_root.visual._paint[0].albedo_color == game.player_color(1), "hotseat handoff applies the next driver's stable paint")
	_check(race.car_root.get_instance_id() != old and race.car_root.current_gear == 0 and race.car_root.wheel_array.size() == 4 and race.car_root.axles.size() == 2 and race.car_root.linear_velocity == Vector3.ZERO, "handoff recreates all hidden drivetrain/suspension state")
	_check(race.car_root.front_torque_split == 0.0, "hotseat retains selected car configuration")
	for wheel: Wheel in race.car_root.wheel_array:
		_check(wheel.spin == 0 and wheel.previous_global_position.is_equal_approx(wheel.global_position), "fresh wheels have no stale spin or position history")
	race.car_root.global_position += Vector3.UP * 8
	await _frames(24)
	_check(race.recovery_count == 0, "ordinary flight never triggers recovery")
	race.car_root.global_position = Vector3(10000, 10000, 10000)
	await _frames(3)
	_check(race.recovery_count == 1 and race.car_root.global_position.distance_to(race._spawn_pose.origin) < 2, "outside finite world triggers full retry")
	_check(race.car_root.visual._paint[0].albedo_color == game.player_color(1), "recovery preserves the active driver's paint")
	race.set_paused(true)
	race.pause_menu.get_node("%Setup").pressed.emit()
	_check(app.content.has_node("%Players") and not game.paused and not paused, "pause-to-setup clears tree pause")
	app.content.get_node("%Back").pressed.emit()
	_check(app.content.has_node("%Quit"), "back returns to menu")
	game.car_scene = saved_car
	await _dispose([app])
