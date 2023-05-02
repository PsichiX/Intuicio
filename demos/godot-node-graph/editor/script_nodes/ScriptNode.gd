class_name ScriptNode
extends GraphNode

signal changed
signal closed

var _data = null
var _input_ports = []
var _output_ports = []

const SLOT_EXECUTE = 0
const SLOT_PROPERTY = 1
const COLOR_EXECUTE = Color.white
const COLOR_PROPERTY = Color.limegreen

func setup(data):
	_data = data
	name = data.id
	broadcast_change()

func data():
	return _data

func id():
	return _data.id if _data else null

func input_name(port):
	return _input_ports[port]

func output_name(port):
	return _output_ports[port]

func input_port(name):
	for index in range(_input_ports.size()):
		if _input_ports[index] == name:
			return index
	return -1

func output_port(name):
	for index in range(_output_ports.size()):
		if _output_ports[index] == name:
			return index
	return -1

func broadcast_change():
	emit_signal("changed", _data)

func add_execute_in(label):
	var slot = get_child_count()
	_input_ports.append(label)
	var node = Label.new()
	node.text = label
	node.align = Label.ALIGN_LEFT
	add_child(node)
	set_slot_enabled_left(slot, true)
	set_slot_type_left(slot, SLOT_EXECUTE)
	set_slot_color_left(slot, COLOR_EXECUTE)

func add_execute_out(label):
	var slot = get_child_count()
	_output_ports.append(label)
	var node = Label.new()
	node.text = label
	node.align = Label.ALIGN_RIGHT
	add_child(node)
	set_slot_enabled_right(slot, true)
	set_slot_type_right(slot, SLOT_EXECUTE)
	set_slot_color_right(slot, COLOR_EXECUTE)

func add_property(label, node_class):
	var slot = get_child_count()
	var node = node_class.new()
	add_child(node)
	set_slot_type_left(slot, SLOT_PROPERTY)
	set_slot_color_left(slot, COLOR_PROPERTY)
	return node

func add_property_bool(label, value = false):
	var node = add_property(label, CheckBox)
	node.text = label
	node.align = HALIGN_LEFT
	node.pressed = value
	return node

func add_property_int(label, signed, value = 0):
	var node = add_property(label, IntegerProperty)
	node.label = label
	node.value = value
	node.signed = signed
	return node

func add_property_float(label, value = 0.0):
	var node = add_property(label, FloatProperty)
	node.label = label
	node.value = value
	return node

func add_property_string(label, single, value = ""):
	var node = add_property(label, TextProperty)
	node.label = label
	node.value = value
	node.single = single
	node.validate()
	return node

func _init():
	add_constant_override("separation", 10)

func _ready():
	title = "Script node"
	show_close = true
	var node = HSeparator.new()
	node.rect_min_size.y = 12
	add_child(node)
	connect("close_request", self, "_closed")
	connect("dragged", self, "_moved")

func _closed():
	emit_signal("closed", _data.id)

func _moved(from, to):
	if _data:
		_data.x = to.x
		_data.y = to.y
	broadcast_change()
