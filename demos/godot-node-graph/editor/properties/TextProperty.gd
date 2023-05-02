class_name TextProperty
extends HBoxContainer

signal changed

export(String) var label = "" setget set_label, get_label
export(String) var value = "" setget set_value, get_value
export(bool) var single = false setget set_single, is_single

var _label_node = null
var _text_node = null

func validate():
	if single:
		if value.empty():
			set_value("@")
		else:
			set_value(value.left(1))
	

func set_label(v):
	label = v
	_label_node.text = label

func get_label():
	return label

func set_value(v):
	if v == value:
		return
	
	if single:
		if v.empty():
			v = "@"
		else:
			v = v.left(1)
	
	value = v
	_text_node.text = value

func get_value():
	return value

func set_single(v):
	single = v
	_text_node.max_length = 1 if single else 0
	if single:
		set_value(value.left(1))

func is_single():
	return single

func _ready():
	_label_node = Label.new()
	_label_node.text = label
	_label_node.size_flags_horizontal = SIZE_EXPAND | SIZE_FILL
	add_child(_label_node)
	
	_text_node = LineEdit.new()
	_text_node.expand_to_text_length = true
	_text_node.text = value
	_text_node.max_length = 1 if single else 0
	_text_node.caret_blink = true
	_text_node.connect("text_entered", self, "_changed")
	_text_node.connect("focus_exited", self, "_canceled")
	add_child(_text_node)

func _changed(v):
	set_value(v)
	emit_signal("changed", value)

func _canceled():
	emit_signal("changed", value)
