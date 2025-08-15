// Example cuenv configuration with preload hooks for testing Starship module
package env

import "github.com/rawkode/cuenv/schema"

schema.#Cuenv

env: {
    STARSHIP_TEST: "true"
}

hooks: {
    onEnter: [
        // Quick test hook (2 seconds)
        {
            command: "sleep"
            args: ["2"]
            preload: true
        },
        // Medium duration hook (5 seconds)
        {
            command: "sh"
            args: ["-c", "echo 'Starting environment setup...' && sleep 5"]
            preload: true
        },
        // Long-running hook (10 seconds) - optional, uncomment to test
        // {
        //     command: "sh"
        //     args: ["-c", "for i in $(seq 1 10); do echo \"Progress: $i/10\"; sleep 1; done"]
        //     preload: true
        // },
    ]
}
