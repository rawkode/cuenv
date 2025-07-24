package env

env: {
    // This won't be masked - it's just a plain string
    PLAIN_SECRET: "mysecretvalue"
    
    // This should be masked - it uses the secret resolver syntax
    RESOLVED_SECRET: {
        resolver: {
            command: "echo"
            args: ["mysecretvalue"]
        }
    }
    
    // Normal non-secret value
    NORMAL_VAR: "hello"
}