class_name ScriptBranchScopeNode
extends ScriptNode

func _ready():
	title = "Branch scope"
	add_execute_in("In")
	add_execute_out("Out")
	add_execute_out("Success body")
	add_execute_out("Failure body")
