class_name IntegerProperty
extends HBoxContainer

signal changed

export(String) var label = "" setget set_label, get_label
export(int) var value = 0 setget set_value, get_value
export(bool) var signed = true setget set_signed, is_signed

var _label_node = null
var _spinbox_node = null

func set_label(v):
	label = v
	_label_node.text = label

func get_label():
	return label

func set_value(v):
	value = v
	_spinbox_node.value = value

func get_value():
	return value

func set_signed(v):
	signed = v
	_spinbox_node.allow_lesser = signed

func is_signed():
	return signed

func _ready():
	_label_node = Label.new()
	_label_node.text = label
	_label_node.size_flags_horizontal = SIZE_EXPAND | SIZE_FILL
	add_child(_label_node)
	
	_spinbox_node = SpinBox.new()
	_spinbox_node.value = value
	_spinbox_node.allow_greater = true
	_spinbox_node.allow_lesser = signed
	_spinbox_node.rounded = true
	_spinbox_node.connect("value_changed", self, "_changed")
	_spinbox_node.connect("focus_exited", self, "_canceled")
	add_child(_spinbox_node)

func _changed(v):
	set_value(v)
	emit_signal("changed", value)

func _canceled():
	emit_signal("changed", value)
