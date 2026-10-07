extends Control

signal start_hotseat
signal quit_game

func _ready() -> void:
	%Start.pressed.connect(func() -> void: start_hotseat.emit())
	%Quit.pressed.connect(func() -> void: quit_game.emit())
	%Start.grab_focus()
