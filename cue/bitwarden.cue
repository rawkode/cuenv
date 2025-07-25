package cuenv

#BitwardenRef: #Resolver & {
	itemId: string
	field:  string | *"password"
	ref:    "bitwarden://\(itemId)/\(field)"
	resolver: #ExecResolver & {
		command: "bw"
		args: [
			"get", field,
			itemId,
		]
	}
}

#BitwardenItemRef: #Resolver & {
	name:  string
	field: string | *"password"
	ref:   "bitwarden-item://\(name)/\(field)"
	resolver: #ExecResolver & {
		command: "sh"
		args: [
			"-c",
			"bw get item \"\(name)\" | jq -r '.login.\(field) // .fields[] | select(.name==\"\(field)\") | .value'",
		]
	}
}