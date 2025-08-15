package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
	SCRIPTS_DIR:    "./scripts"
	SHELL:          "/bin/bash"
	SCRIPT_TIMEOUT: "300"
	DEBUG_SCRIPTS:  "false"
}
