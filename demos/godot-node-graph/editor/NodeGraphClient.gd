class_name NodeGraphClient
extends Node

export var port = 8001

signal response
signal response_error
signal response_create
signal response_destroy
signal response_list
signal response_suggest_all_nodes
signal response_add
signal response_remove
signal response_update
signal response_query_all
signal response_serialize
signal response_deserialize
signal change

const SERVER_PATH_DEBUG = "res://../../../target/debug/godot-node-graph-server.exe"
const SERVER_PATH_RELEASE = "res://godot-node-graph-server.exe"

var _client = WebSocketClient.new()
var _state = 0
var _server = -1

func request(data):
	var content = JSON.print(data)
#	print("* Request: %s" % content)
	_client.get_peer(1).put_packet(content.to_utf8())

func request_create():
	request({"Create": {}})

func request_destroy(graph):
	request({"Destroy": {"graph": graph}})

func request_list():
	request({"List": {}})
	
func request_suggest_all_nodes(x, y):
	request({"SuggestAllNodes": {"x": x, "y": y}})

func request_add(graph, nodes = [], connections = []):
	request({"Add": {
		"graph": graph,
		"content": {
			"nodes": nodes,
			"connections": connections,
		},
	}})

func request_remove(graph, nodes = [], connections = []):
	request({"Remove": {
		"graph": graph,
		"content": {
			"nodes": nodes,
			"connections": connections,
		},
	}})

func request_update(graph, nodes = [], connections = []):
	request({"Update": {
		"graph": graph,
		"content": {"nodes": nodes},
	}})

func request_query_all(graph):
	request({"QueryAll": {"graph": graph}})

func request_serialize(graph):
	request({"Serialize": {"graph": graph}})

func request_deserialize(graph, content):
	request({"Deserialize": {"graph": graph, "content": content}})

func request_validate(graph):
	request({"Validate": {"graph": graph}})

func is_alive():
	return _state == 2

func is_pending():
	return _state == 1

func is_dead():
	return _state == 0

func restart():
	_client.disconnect_from_host()

func _ready():
	_client.connect("connection_closed", self, "_closed")
	_client.connect("connection_error", self, "_closed")
	_client.connect("connection_established", self, "_connected")
	_client.connect("data_received", self, "_on_data")
	var path = _server_path()
	var args = [str(port)]
	if OS.has_feature("debug"):
		args.append("--verbose")
	_server = OS.execute(
		path,
		args,
		false,
		[],
		false,
		OS.has_feature("debug")
	)
	print("Run server: %s on port: %s. Process id: %s" % [
		path,
		port,
		_server,
	])

func _exit_tree():
	if OS.kill(_server):
		print("Failed to kill server process: %s" % _server)
	else:
		print("Killed server process: %s" % _server)
	_server = -1

func _closed(was_clean = false):
	_state = 0
	emit_signal("change", is_alive())
	print("Disconnected from node graph server")

func _connected(proto = ""):
	_state = 2
	emit_signal("change", is_alive())
	print("Connected to node graph server")

func _on_data():
	var content = _client.get_peer(1).get_packet().get_string_from_utf8()
#	print("* Response: %s" % content)
	var result = JSON.parse(content)
	if result.error == OK:
		emit_signal("response", result.result)
		if "Error" in result.result:
			emit_signal("response_error", result.result.Error)
		elif "Create" in result.result:
			emit_signal("response_create", result.result.Create)
		elif "Destroy" in result.result:
			emit_signal("response_destroy", result.result.Destroy)
		elif "List" in result.result:
			emit_signal("response_list", result.result.List)
		elif "SuggestAllNodes" in result.result:
			emit_signal("response_suggest_all_nodes", result.result.SuggestAllNodes)
		elif "Add" in result.result:
			emit_signal("response_add", result.result.Add)
			request_query_all(result.result.Add.graph)
		elif "Remove" in result.result:
			emit_signal("response_remove", result.result.Remove)
			request_query_all(result.result.Remove.graph)
		elif "Update" in result.result:
			emit_signal("response_update", result.result.Update)
			request_query_all(result.result.Update.graph)
		elif "QueryAll" in result.result:
			emit_signal("response_query_all", result.result.QueryAll)
		elif "Serialize" in result.result:
			emit_signal("response_serialize", result.result.Serialize)
		elif "Deserialize" in result.result:
			emit_signal("response_deserialize", result.result.Deserialize)
			request_query_all(result.result.Deserialize.graph)

func _process(delta):
	if _state == 0:
		print("Try connect to node graph server")
		var result = _client.connect_to_url("ws://127.0.0.1:%s" % port, [])
		if result == OK:
			_state = 1
			print("Connecting to node graph server")
		else:
			print("Could not connect to node graph server")
	else:
		_client.poll()

func _server_path():
	var result = SERVER_PATH_RELEASE
	if OS.has_feature("debug"):
		result = SERVER_PATH_DEBUG
	return ProjectSettings.globalize_path(result)
