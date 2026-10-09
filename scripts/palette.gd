class_name Palette
extends RefCounted

const SOURCE: AuthoringPalette = preload("res://resources/authoring_palette.tres")
const PLAYER_INDICES := [8, 17, 12, 10, 28, 18, 9, 19, 13, 27, 0, 20, 11, 29, 4, 22]

static func color(index: int, alpha := 1.0) -> Color:
	return SOURCE.color(index, alpha)

static func player_index(driver: int) -> int:
	assert(driver >= 0 and driver < PLAYER_INDICES.size())
	return PLAYER_INDICES[driver]
