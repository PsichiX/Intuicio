class_name ScriptPushScopeNode
extends ScriptNode

func _ready():
	title = "Push scope"
	add_execute_in("In")
	add_execute_out("Out")
	add_execute_out("Body")
