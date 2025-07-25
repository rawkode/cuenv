package cuenv

#PassRef: #Resolver & {
	path: string
	ref:  "pass://\(path)"
	resolver: #ExecResolver & {
		command: "pass"
		args: [path]
	}
}

#PassFieldRef: #Resolver & {
	path:  string
	field: string
	ref:   "pass-field://\(path)/\(field)"
	resolver: #ExecResolver & {
		command: "pass"
		args: [path, "|", "grep", "^\(field):", "|", "cut", "-d:", "-f2-", "|", "sed", "s/^ *//"]
	}
}