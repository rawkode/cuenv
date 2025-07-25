package cuenv

#VaultRef: #Resolver & {
	path:  string
	field: string
	ref:   "vault://\(path)/\(field)"
	resolver: #ExecResolver & {
		command: "vault"
		args: [
			"kv", "get", "-field=\(field)",
			path,
		]
	}
}

#VaultDynamicRef: #Resolver & {
	path:  string
	field: string
	ref:   "vault-dynamic://\(path)/\(field)"
	resolver: #ExecResolver & {
		command: "vault"
		args: [
			"read", "-field=\(field)",
			path,
		]
	}
}