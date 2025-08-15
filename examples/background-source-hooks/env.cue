package examples

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

hooks: {
	onEnter: [
		{
			// This hook sleeps for 5 seconds then exports TEST_BG_VAR
			command: "bash"
			args: ["-c", """
				sleep 5
				echo 'export TEST_BG_VAR="background_hook_completed"'
				echo 'export TEST_TIMESTAMP="'$(date +%s)'"'
				"""]
			source: true  // Capture the exported environment
		},
	]
}

env: {
	TEST_ENV: "background_source_test"
}
