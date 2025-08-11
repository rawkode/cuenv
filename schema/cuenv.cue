package schema

#Cuenv: {
	capabilities?: [string]: #Capability
	env?: #Env
	hooks?: #Hooks
	tasks: [string]: #Tasks | *{}
}
