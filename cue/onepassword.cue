package cuenv

#OnePasswordRef: #Resolver & {
	ref: string
	resolver: #ExecResolver & {
		command: "op"
		args: [
			"read",
			ref,
		]
	}
}
