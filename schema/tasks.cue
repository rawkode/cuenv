package schema

#Tasks: {
	description: string | *"No description provided"
	#TaskGroup | #Task
}

#Task: {
	shell: string | *"bash"
	command!: string
	args?: [...string]

	dependencies?: [...string]
	inputs?: [...string]
	outputs?: [...string]
}

// Execution modes for task groups:
// - workflow: Execute based on dependency graph (DAG)
// - sequential: Execute tasks one after another in order
// - parallel: Execute all tasks simultaneously  
// - group: Organization only, no group execution (must run individual tasks)
#TaskGroupMode: "workflow" | "sequential" | "parallel" | "group"

#TaskGroup: {
	mode?: #TaskGroupMode | *"group"
	[string]: #Tasks
}
