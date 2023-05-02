extends Control

onready var _client = $Client
onready var _editor = $View/GraphEdit
onready var _suggestions = $Popups/SuggestionsPopup
onready var _new_button = $View/Buttons/New
onready var _save_button = $View/Buttons/Save
onready var _load_button = $View/Buttons/Load

var _graph = null
var _pending_connection = null

const UUID = preload("res://uuid/uuid.gd")

class PendingConnection extends Reference:
	var node_id: String
	var source_pin: String
	var target_pin: String
	
	func _init(node_id, source_pin, target_pin):
		self.node_id = node_id
		self.source_pin = source_pin
		self.target_pin = target_pin

func _init():
	OS.low_processor_usage_mode = true

func _ready():
	_editor.scroll_offset = _editor.rect_size * -0.4
	_client.connect("change", self, "_client_change")
	_client.connect("response_create", self, "_client_response_create")
	_client.connect("response_suggest_all_nodes", self, "_client_response_suggest_all_nodes")
	_client.connect("response_query_all", self, "_client_response_query_all")
	_client.connect("response_serialize", self, "_client_response_serialize")
	_editor.connect("popup_request", self, "_editor_suggestions")
	_editor.connect("connection_request", self, "_editor_bind_nodes")
	_editor.connect("disconnection_request", self, "_editor_unbind_nodes")
	_editor.connect("connection_from_empty", self, "_editor_bind_node_input")
	_editor.connect("connection_to_empty", self, "_editor_bind_node_output")
	_suggestions.connect("create_node", self, "_suggestions_create_node")
	_suggestions.connect("modal_closed", self, "_suggestions_close")
	_new_button.connect("pressed", self, "_new")
	_save_button.connect("pressed", self, "_save")
	_load_button.connect("pressed", self, "_load")

func editor_node(name):
	for child in _editor.get_children():
		if child.get_class() == "GraphNode" && child.name == name:
			return child
	return null

func editor_node_named(id):
	for child in _editor.get_children():
		if child.get_class() == "GraphNode" && child.id() == id:
			return child
	return null

func _client_change(alive):
	if alive:
		_client.request_create()
	else:
		_graph = null

func _client_response_create(data):
	_graph = data.graph
	_client.request_add(_graph, [{
		"id": UUID.v4(),
		"x": 0,
		"y": 0,
		"data": "Start",
	}])

func _client_response_suggest_all_nodes(data):
	_suggestions.show_items(get_global_mouse_position(), data.content)

func _client_response_query_all(data):
	_editor.clear_connections()
	for child in _editor.get_children():
		if child.get_class() == "GraphNode":
			_editor.remove_child(child)
			child.queue_free()
	for item in data.content.nodes:
		var node = null
		if typeof(item.data) == TYPE_STRING && item.data == "Start":
			node = ScriptStartNode
		elif "Operation" in item.data:
			var operation = item.data.Operation
			if "Expression" in operation:
				var expression = operation.Expression;
				if "Literal" in expression:
					var literal = expression.Literal
					if "Unit" in literal:
						node = ScriptLiteralUnitNode
					elif "Bool" in literal:
						node = ScriptLiteralBoolNode
					elif "I8" in literal:
						node = ScriptLiteralI8Node
					elif "I16" in literal:
						node = ScriptLiteralI16Node
					elif "I32" in literal:
						node = ScriptLiteralI32Node
					elif "I64" in literal:
						node = ScriptLiteralI64Node
					elif "I128" in literal:
						node = ScriptLiteralI128Node
					elif "Isize" in literal:
						node = ScriptLiteralIsizeNode
					elif "U8" in literal:
						node = ScriptLiteralU8Node
					elif "U16" in literal:
						node = ScriptLiteralU16Node
					elif "U32" in literal:
						node = ScriptLiteralU32Node
					elif "U64" in literal:
						node = ScriptLiteralU64Node
					elif "U128" in literal:
						node = ScriptLiteralU128Node
					elif "Usize" in literal:
						node = ScriptLiteralUsizeNode
					elif "F32" in literal:
						node = ScriptLiteralF32Node
					elif "F64" in literal:
						node = ScriptLiteralF64Node
					elif "Char" in literal:
						node = ScriptLiteralCharNode
					elif "String" in literal:
						node = ScriptLiteralStringNode
				elif typeof(expression) == TYPE_STRING && expression == "StackDrop":
					node = ScriptStackDropNode
			elif "MakeRegister" in operation:
				node = ScriptMakeRegisterNode
			elif "DropRegister" in operation:
				node = ScriptDropRegisterNode
			elif "PushFromRegister" in operation:
				node = ScriptPushFromRegisterNode
			elif "PopToRegister" in operation:
				node = ScriptPopToRegisterNode
			elif "CallFunction" in operation:
				node = ScriptCallFunctionNode
			elif "BranchScope" in operation:
				node = ScriptBranchScopeNode
			elif "LoopScope" in operation:
				node = ScriptLoopScopeNode
			elif "PushScope" in operation:
				node = ScriptPushScopeNode
			elif typeof(operation) == TYPE_STRING && operation == "PopScope":
				node = ScriptPopScopeNode
		if node:
			node = node.new()
			node.offset.x = item.x
			node.offset.y = item.y
			node.setup(item)
			node.connect("closed", self, "_editor_destroy_node")
			node.connect("changed", self, "_editor_update_node")
			_editor.add_child(node)
	for connection in data.content.connections:
		var from_node = editor_node_named(connection.from_node)
		var to_node = editor_node_named(connection.to_node)
		var from_port = from_node.output_port(connection.from_pin)
		var to_port = to_node.input_port(connection.to_pin)
		_editor.connect_node(from_node.name, from_port, to_node.name, to_port)

func _client_response_serialize(data):
	var save_game = File.new()
	save_game.open("user://graph.json", File.WRITE)
	save_game.store_line(to_json(data.content))
	save_game.close()

func _editor_suggestions(position):
	position = position + _editor.scroll_offset
	_client.request_suggest_all_nodes(int(position.x), int(position.y))

func _editor_destroy_node(id):
	_client.request_remove(_graph, [id])

func _editor_update_node(node):
	_client.request_update(_graph, [node])

func _editor_bind_nodes(from, from_slot, to, to_slot):
	var from_pin = editor_node(from).output_name(from_slot)
	var to_pin = editor_node(to).input_name(to_slot)
	_client.request_add(_graph, [], [{
		"from_node": from,
		"to_node": to,
		"from_pin": from_pin,
		"to_pin": to_pin,
	}])

func _editor_unbind_nodes(from, from_slot, to, to_slot):
	var from_pin = editor_node(from).output_name(from_slot)
	var to_pin = editor_node(to).input_name(to_slot)
	_client.request_remove(_graph, [], [{
		"from_node": from,
		"to_node": to,
		"from_pin": from_pin,
		"to_pin": to_pin,
	}])

func _editor_bind_node_input(to, to_slot, position):
	var to_pin = editor_node_named(to).input_name(to_slot)
	_pending_connection = PendingConnection.new(to, to_pin, "Out")
	position = position + _editor.scroll_offset
	_client.request_suggest_all_nodes(int(position.x), int(position.y))

func _editor_bind_node_output(from, from_slot, position):
	var from_pin = editor_node_named(from).output_name(from_slot)
	_pending_connection = PendingConnection.new(from, from_pin, "In")
	position = position + _editor.scroll_offset
	_client.request_suggest_all_nodes(int(position.x), int(position.y))

func _suggestions_create_node(node):
	var connections = []
	if _pending_connection:
		connections.append({
			"from_node": _pending_connection.node_id,
			"to_node": node.id,
			"from_pin": _pending_connection.source_pin,
			"to_pin": _pending_connection.target_pin,
		})
	_client.request_add(_graph, [node], connections)
	_pending_connection = null

func _suggestions_close():
	_pending_connection = null

func _new():
	_client.restart()

func _save():
	_client.request_serialize(_graph)

func _load():
	var save_game = File.new()
	save_game.open("user://graph.json", File.READ)
	var data = parse_json(save_game.get_line())
	save_game.close()
	_client.request_deserialize(_graph, data)
