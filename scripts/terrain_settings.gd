class_name TerrainSettings
extends Resource

@export_group("Terrain shape")
@export_range(0.0, 20.0, 0.5) var amplitude := 16.0
@export_range(150.0, 1000.0, 10.0) var wavelength := 220.0
@export_group("Road grade")
@export_range(0.01, 0.10, 0.005) var max_gradient := 0.10
@export_group("Shoulders")
@export_range(16.0, 64.0, 1.0) var blend_distance := 16.0

func valid() -> bool:
	return is_finite(amplitude) and amplitude >= 0 and amplitude <= 20 and is_finite(wavelength) and wavelength >= 150 and wavelength <= 1000 and is_finite(max_gradient) and max_gradient >= 0.01 and max_gradient <= 0.10 and is_finite(blend_distance) and blend_distance >= 16 and blend_distance <= 64
