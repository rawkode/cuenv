package gcp

import "github.com/rawkode/cuenv"

#GcpSecret: cuenv.#Resolver & {
	project: string
	secret: string
	version: string | *"latest"
	ref: "gcp://\(project)/\(secret)/\(version)"
	resolver: cuenv.#ExecResolver & {
		command: "gcloud"
		args: [
			"secrets", "versions", "access", version,
			"--secret", secret,
			"--project", project,
		]
	}
}