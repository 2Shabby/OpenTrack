extends "res://tools/authoring_diagnostics/handling_probe.gd"

func _run() -> void:
	studio = Node3D.new()
	root.add_child(studio)
	floor_body = StaticBody3D.new()
	floor_body.collision_layer = TrackGeometry.TERRAIN_LAYER
	floor_body.position.y = -0.5
	studio.add_child(floor_body)
	var collider := CollisionShape3D.new()
	collider.shape = BoxShape3D.new()
	collider.shape.size = Vector3(200, 1, 8000)
	floor_body.add_child(collider)
	var road := StaticBody3D.new()
	road.collision_layer = TrackGeometry.ROAD_LAYER
	road.add_to_group("Road")
	road.position = Vector3(0, -0.48, -1000)
	studio.add_child(road)
	var road_collider := CollisionShape3D.new()
	road_collider.shape = BoxShape3D.new()
	road_collider.shape.size = Vector3(200, 1, 2060)
	road.add_child(road_collider)
	await _spawn("Grass")
	car.accept_input = true
	Input.action_press("throttle_positive")
	var mixed := false
	var min_contacts := 4
	var samples: Array[Dictionary] = []
	for i in 720:
		await physics_frame
		var data := car.telemetry()
		mixed = mixed or ("Road" in data.wheels and "Grass" in data.wheels)
		min_contacts = mini(min_contacts, data.contacts)
		assert(car.linear_velocity.is_finite() and car.angular_velocity.is_finite())
		if i % 120 == 119:
			samples.append({"second": (i + 1) / 120, "speed": data.signed_speed, "surface": data.surface, "wheels": data.wheels})
	Input.action_release("throttle_positive")
	var start_speed := car.linear_velocity.length()
	await _frames(360)
	var coast_speed := car.linear_velocity.length()
	Input.action_press("throttle_negative")
	await _frames(1200)
	Input.action_release("throttle_negative")
	var reverse := car.telemetry()
	assert(mixed and min_contacts >= 3 and reverse.gear == -1 and reverse.signed_speed < -1.0)
	road.free()
	floor_body.rotation.x = deg_to_rad(-6.0)
	await _spawn("Grass")
	car.throttle_input = 1.0
	await _frames(720)
	var uphill := car.telemetry()
	assert(uphill.contacts >= 3 and uphill.signed_speed > 2.0)
	print("GRASS_TRANSITION_PROBE ", JSON.stringify({"mixed_contacts_seen": mixed, "minimum_contacts": min_contacts, "transition": samples, "grass_coast_from_mps": start_speed, "grass_coast_after_3_mps": coast_speed, "reverse": reverse, "grass_uphill_6deg_after_6sec": uphill}))
	studio.free()
	quit()
