class_name ScriptLoopScopeNode
extends ScriptNode

func _ready():
	title = "Loop scope"
	add_execute_in("In")
	add_execute_out("Out")
	add_execute_out("Iteration body")
