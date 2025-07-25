package cuenv

#AWSSecretsRef: #Resolver & {
	secretId: string
	region?:  string
	ref:      "aws-secrets://\(secretId)"
	resolver: #ExecResolver & {
		command: "aws"
		args: [
			"secretsmanager", "get-secret-value",
			"--secret-id", secretId,
			"--query", "SecretString",
			"--output", "text",
		] + (if region != _|_ {["--region", region]} else [])
	}
}

#AWSSecretsJSONRef: #Resolver & {
	secretId: string
	jsonKey:  string
	region?:  string
	ref:      "aws-secrets-json://\(secretId)/\(jsonKey)"
	resolver: #ExecResolver & {
		command: "sh"
		args: [
			"-c",
			"aws secretsmanager get-secret-value --secret-id \(secretId)" +
			(if region != _|_ {" --region \(region)"} else "") +
			" --query SecretString --output text | jq -r .\(jsonKey)",
		]
	}
}