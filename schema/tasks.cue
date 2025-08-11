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

#TaskGroup: {
	[string]: #Tasks
}
