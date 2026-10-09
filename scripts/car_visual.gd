class_name CarVisual
extends Node3D

# Generated visual scenes bind these roles explicitly, independent of asset names.
@export var wheels: Array[MeshInstance3D] = [] # FL, FR, RL, RR or Front, RL, RR
@export var wheel_radii := PackedFloat32Array()
@export var wheel_widths := PackedFloat32Array()
@export var tail_lamps: Array[MeshInstance3D] = []
@export var brake_lamps: Array[MeshInstance3D] = []
@export var reverse_lamps: Array[MeshInstance3D] = []

var _paint: Array[StandardMaterial3D] = []
var _tail: Array[StandardMaterial3D] = []
var _brake: Array[StandardMaterial3D] = []
var _reverse: Array[StandardMaterial3D] = []
var _materials_prepared := false

func validation_error() -> String:
	var count := wheels.size()
	if count not in [3, 4] or wheel_radii.size() != count or wheel_widths.size() != count:
		return "CarVisual requires FL/FR/RL/RR or Front/RL/RR wheel bindings and dimensions."
	var front_count := count - 2
	var unique := {}
	for i in count:
		var wheel := wheels[i]
		if not is_instance_valid(wheel) or wheel.mesh == null or not is_ancestor_of(wheel) or unique.has(wheel):
			return "CarVisual wheel bindings must reference distinct descendant meshes."
		unique[wheel] = true
		if not is_finite(wheel_radii[i]) or not is_finite(wheel_widths[i]) or wheel_radii[i] <= 0 or wheel_widths[i] <= 0:
			return "CarVisual wheel dimensions must be positive metres."
		var pose := _local_pose(wheel)
		if not pose.basis.is_equal_approx(Basis.IDENTITY):
			return "Wheel meshes must have positive unit scale, zero rotation and hub-centred geometry."
		if wheel.mesh.get_aabb().get_center().length() > 0.001:
			return "Wheel mesh origins must be at their hub centres."
		if count == 3 and i == 0:
			if not is_zero_approx(pose.origin.x):
				return "A three-wheeler requires a centered front wheel."
		elif is_zero_approx(pose.origin.x) or (pose.origin.x < 0) != ((i if count == 4 else i - 1) % 2 == 0):
			return "Wheel bindings must be ordered FL/FR/RL/RR or Front/RL/RR in -Z-forward coordinates."
	for i in ([0, 2] if count == 4 else [1]):
		if not is_equal_approx(wheel_radii[i], wheel_radii[i + 1]) or not is_equal_approx(wheel_widths[i], wheel_widths[i + 1]):
			return "GEVP requires matching tire dimensions on each axle."
	if _local_pose(wheels[0]).origin.z >= _local_pose(wheels[front_count]).origin.z:
		return "Front wheel mounts must precede rear mounts along -Z."
	if not is_equal_approx(_local_pose(wheels[count - 2]).origin.z, _local_pose(wheels[count - 1]).origin.z) or (count == 4 and not is_equal_approx(_local_pose(wheels[0]).origin.z, _local_pose(wheels[1]).origin.z)):
		return "Left and right wheels on each axle must share the same longitudinal mount."
	for lamps in [tail_lamps, brake_lamps, reverse_lamps]:
		if lamps.is_empty():
			return "CarVisual requires tail, brake and reverse lamp bindings."
		for lamp in lamps:
			if not is_instance_valid(lamp) or lamp.mesh == null or not is_ancestor_of(lamp) or unique.has(lamp):
				return "Lamp bindings must reference distinct descendant meshes."
			unique[lamp] = true
			for surface in lamp.mesh.get_surface_count():
				if not lamp.get_active_material(surface) is StandardMaterial3D:
					return "Lamp meshes require StandardMaterial3D materials."
	var has_paint := false
	for mesh: MeshInstance3D in find_children("*", "MeshInstance3D", true, false):
		if mesh.mesh == null:
			return "CarVisual mesh bindings require mesh resources."
		for surface in mesh.mesh.get_surface_count():
			var material := mesh.get_active_material(surface)
			has_paint = has_paint or (material is StandardMaterial3D and material.resource_name == "Paint")
	if not has_paint:
		return "CarVisual requires a material with the semantic role Paint."
	return ""

func configure(palette_index: int) -> void:
	var color := Palette.color(palette_index)
	if not _materials_prepared:
		_collect_paint(self)
		_tail = _lamp_materials(tail_lamps, Palette.color(7))
		_brake = _lamp_materials(brake_lamps, Palette.color(8))
		_reverse = _lamp_materials(reverse_lamps, Palette.color(19))
		_materials_prepared = true
		set_lights(0.0, false)
	for material in _paint:
		material.albedo_color = color

func wheel_bindings() -> Array[Dictionary]:
	var bindings: Array[Dictionary] = []
	for i in wheels.size():
		bindings.append({"pivot": wheels[i], "center": _local_pose(wheels[i]).origin, "radius": wheel_radii[i], "width": wheel_widths[i]})
	return bindings

func set_lights(braking: float, reversing: bool) -> void:
	for material in _tail:
		material.emission_energy_multiplier = 0.35
	for material in _brake:
		material.emission_energy_multiplier = 3.0 * clampf(braking, 0.0, 1.0)
	for material in _reverse:
		material.emission_energy_multiplier = 1.5 if reversing else 0.0

# Compose local transforms before scene entry; no global-transform dependency.
func _local_pose(node: Node3D) -> Transform3D:
	var pose := node.transform
	var ancestor := node.get_parent() as Node3D
	while ancestor != self:
		pose = ancestor.transform * pose
		ancestor = ancestor.get_parent() as Node3D
	return transform * pose

func _collect_paint(node: Node) -> void:
	if node is MeshInstance3D:
		for surface in node.mesh.get_surface_count():
			var source: Material = node.get_active_material(surface)
			if source is StandardMaterial3D and source.resource_name == "Paint":
				var material := source.duplicate() as StandardMaterial3D
				node.set_surface_override_material(surface, material)
				_paint.append(material)
	for child in node.get_children():
		_collect_paint(child)

func _lamp_materials(lamps: Array[MeshInstance3D], color: Color) -> Array[StandardMaterial3D]:
	var materials: Array[StandardMaterial3D] = []
	for lamp in lamps:
		for surface in lamp.mesh.get_surface_count():
			var material := lamp.get_active_material(surface).duplicate() as StandardMaterial3D
			material.emission_enabled = true
			material.emission = color
			lamp.set_surface_override_material(surface, material)
			materials.append(material)
	return materials
