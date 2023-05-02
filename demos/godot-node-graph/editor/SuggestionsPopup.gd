extends PopupMenu

signal create_node

onready var _literal_popup = $LiteralPopup
onready var _register_popup = $RegisterPopup
onready var _scope_popup = $ScopePopup

var _literal_items = []
var _register_items = []
var _scope_items = []
var _misc_items = []

const ID_LITERAL = 0
const ID_REGISTER = 1
const ID_SCOPE = 2
const ID_MISC = 3

func show_items(position, data):
	_literal_items.clear()
	_register_items.clear()
	_scope_items.clear()
	_misc_items.clear()
	
	_literal_popup.clear()
	_register_popup.clear()
	_scope_popup.clear()
	clear()
	
	add_submenu_item("Literal", "LiteralPopup", ID_LITERAL)
	add_submenu_item("Register", "RegisterPopup", ID_REGISTER)
	add_submenu_item("Scope", "ScopePopup", ID_SCOPE)
	for item in data:
		if item.category == "Literal":
			_literal_items.append(item.node)
			_literal_popup.add_item(item.label)
		elif item.category == "Register":
			_register_items.append(item.node)
			_register_popup.add_item(item.label)
		elif item.category == "Scope":
			_scope_items.append(item.node)
			_scope_popup.add_item(item.label)
		else:
			_misc_items.append(item.node)
			add_item(item.label)
			
	set_as_minsize()
	popup(Rect2(position, rect_size))

func _ready():
	_literal_popup.connect("id_pressed", self, "_literal_pressed")
	_register_popup.connect("id_pressed", self, "_register_pressed")
	_scope_popup.connect("id_pressed", self, "_scope_pressed")
	connect("id_pressed", self, "_misc_pressed")

func _literal_pressed(id):
	emit_signal("create_node", _literal_items[id])

func _register_pressed(id):
	emit_signal("create_node", _register_items[id])

func _scope_pressed(id):
	emit_signal("create_node", _scope_items[id])

func _misc_pressed(id):
	id = id - ID_MISC
	emit_signal("create_node", _misc_items[id])
