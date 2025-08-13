package main

import "cuenv.org/env"

env: {
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
	
	environment: {
		TEST_ENV: "background_source_test"
	}
}