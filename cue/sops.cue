package cuenv

#SOPSRef: #Resolver & {
	file:   string
	path:   string
	format: "yaml" | "json" | *"yaml"
	ref:    "sops://\(file)/\(path)"
	resolver: #ExecResolver & {
		command: "sops"
		args: [
			"--decrypt", "--extract", "[\"\(path)\"]",
			file,
		]
	}
}

#SOPSJSONRef: #Resolver & {
	file:    string
	jsonKey: string
	ref:     "sops-json://\(file)/\(jsonKey)"
	resolver: #ExecResolver & {
		command: "sh"
		args: [
			"-c",
			"sops --decrypt \(file) | jq -r .\(jsonKey)",
		]
	}
}