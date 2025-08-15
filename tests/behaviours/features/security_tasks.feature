Feature: Security Tasks
  As a developer
  I want to execute tasks with security restrictions
  So that I can safely run untrusted code with controlled access

  Background:
    Given I have a temporary test directory

  Scenario: Execute secure build task with filesystem restrictions
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "secure-build": {
          description: "Build the project with restricted filesystem access"
          command: "echo"
          args: ["Building project securely..."]
          security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp", "./build"]
          }
        }
      }
      """
    When I run "cuenv task run secure-build"
    Then the output should contain "Building project securely..."
    And the exit code should be 0

  Scenario: Execute network task with host restrictions
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "network-task": {
          description: "Task that needs network access but with restrictions"
          command: "echo"
          args: ["Downloading dependencies..."]
          security: {
            restrictNetwork: true
            allowedHosts: ["api.example.com", "registry.npmjs.org"]
          }
        }
      }
      """
    When I run "cuenv task run network-task"
    Then the output should contain "Downloading dependencies..."
    And the exit code should be 0

  Scenario: Execute fully restricted task
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "fully-restricted": {
          description: "Task with both disk and network restrictions"
          command: "echo"
          args: ["Running in secure sandbox"]
          security: {
            restrictDisk: true
            restrictNetwork: true
            readOnlyPaths: ["/usr/bin", "/bin"]
            readWritePaths: ["/tmp"]
            allowedHosts: ["localhost"]
          }
        }
      }
      """
    When I run "cuenv task run fully-restricted"
    Then the output should contain "Running in secure sandbox"
    And the exit code should be 0

  Scenario: Execute unrestricted task
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "unrestricted": {
          description: "Task without security restrictions"
          command: "echo"
          args: ["Running without restrictions"]
        }
      }
      """
    When I run "cuenv task run unrestricted"
    Then the output should contain "Running without restrictions"
    And the exit code should be 0

  Scenario: List tasks with security information
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "secure-build": {
          description: "Build the project with restricted filesystem access"
          command: "echo"
          args: ["Building project securely..."]
          security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp", "./build"]
          }
        }
        "unrestricted": {
          description: "Task without security restrictions"
          command: "echo"
          args: ["Running without restrictions"]
        }
      }
      """
    When I run "cuenv task list"
    Then the output should contain "secure-build"
    And the output should contain "Build the project with restricted filesystem access"
    And the output should contain "unrestricted"
    And the output should contain "Task without security restrictions"

  Scenario: Task security failure handling with invalid path access
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "restricted-fail": {
          description: "Task that tries to access restricted path"
          command: "ls"
          args: ["/etc/shadow"]
          security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp"]
          }
        }
      }
      """
    When I run "cuenv task run restricted-fail"
    Then the exit code should not be 0

  Scenario: Task with empty security configuration
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        DATABASE_URL: "postgres://localhost/myapp"
        API_KEY: "test-api-key"
        PORT: "3000"
      }
      
      tasks: {
        "empty-security": {
          description: "Task with empty security config"
          command: "echo"
          args: ["Running with empty security"]
          security: {}
        }
      }
      """
    When I run "cuenv task run empty-security"
    Then the output should contain "Running with empty security"
    And the exit code should be 0

  Scenario: Task with environment variables and security restrictions
    Given I have a CUE file "env.cue" with:
      """
      package examples
      
      env: {
        PROJECT_NAME: "secure-app"
        BUILD_PATH: "/tmp/build"
      }
      
      tasks: {
        "secure-env": {
          description: "Secure task with env vars"
          command: "sh"
          args: ["-c", "echo Project: $PROJECT_NAME, Build: $BUILD_PATH"]
          security: {
            restrictDisk: true
            readOnlyPaths: ["/usr", "/lib", "/bin"]
            readWritePaths: ["/tmp"]
          }
        }
      }
      """
    When I run "cuenv task run secure-env"
    Then the output should contain "Project: secure-app"
    And the output should contain "Build: /tmp/build"
    And the exit code should be 0