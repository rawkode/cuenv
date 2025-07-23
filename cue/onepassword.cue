package onepassword

import "github.com/rawkode/cuenv"

#OnePasswordRef: #cuenv.#Resolver & {
	ref: string
	resolver: cuenv.#ExecResolver & {
		command: "op"
		args: [
			"read",
			ref
		]
	}
}
