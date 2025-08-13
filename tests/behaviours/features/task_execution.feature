Feature: Task Execution
  As a developer
  I want to execute tasks defined in CUE
  So that I can automate common development workflows

  Background:
    Given I have a temporary test directory

  Scenario: Execute simple task
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        build: {
          description: "Build the project"
          command: "echo"
          args: ["Building project"]
        }
      }
      
      env: {}
      """
    When I run "cuenv task run build"
    Then the output should contain "Building project"
    And the exit code should be 0

  Scenario: Execute task with dependencies
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        clean: {
          description: "Clean build artifacts"
          command: "echo"
          args: ["Cleaning"]
        }
        build: {
          description: "Build the project"
          depends_on: ["clean"]
          command: "echo"
          args: ["Building"]
        }
      }
      
      env: {}
      """
    When I run "cuenv task run build"
    Then the output should match pattern "Cleaning.*Building"

  Scenario: Execute task group
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        test_unit: {
          description: "Run unit tests"
          command: "echo"
          args: ["Unit tests"]
        }
        test_integration: {
          description: "Run integration tests"
          command: "echo"
          args: ["Integration tests"]
        }
      }
      
      task_groups: {
        test: {
          description: "Run all tests"
          tasks: ["test_unit", "test_integration"]
        }
      }
      
      env: {}
      """
    When I run "cuenv task run test"
    Then the output should contain "Unit tests"
    And the output should contain "Integration tests"

  Scenario: List available tasks
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        build: {
          description: "Build the project"
          command: "true"
        }
        test: {
          description: "Run tests"
          command: "true"
        }
        deploy: {
          description: "Deploy application"
          command: "true"
        }
      }
      
      env: {}
      """
    When I run "cuenv task list"
    Then the output should contain "build"
    And the output should contain "Build the project"
    And the output should contain "test"
    And the output should contain "Run tests"
    And the output should contain "deploy"
    And the output should contain "Deploy application"

  Scenario: Task with environment variables
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        PROJECT_NAME: "myapp"
      }
      
      tasks: {
        info: {
          description: "Show project info"
          command: "sh"
          args: ["-c", "echo Project: $PROJECT_NAME"]
        }
      }
      """
    When I run "cuenv task run info"
    Then the output should contain "Project: myapp"

  Scenario: Task with custom environment
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        custom: {
          description: "Task with custom env"
          command: "sh"
          args: ["-c", "echo $CUSTOM_VAR"]
          env: {
            CUSTOM_VAR: "custom-value"
          }
        }
      }
      
      env: {}
      """
    When I run "cuenv task run custom"
    Then the output should contain "custom-value"

  Scenario: Discover tasks in monorepo
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        root_task: {
          description: "Root level task"
          command: "echo"
          args: ["Root"]
        }
      }
      
      env: {}
      """
    And I have a subdirectory "backend"
    And I have a CUE file "backend/env.cue" with:
      """
      package cuenv
      
      tasks: {
        backend_task: {
          description: "Backend task"
          command: "echo"
          args: ["Backend"]
        }
      }
      
      env: {}
      """
    When I run "cuenv task list --all"
    Then the output should contain "root_task"
    And the output should contain "backend/backend_task"

  Scenario: Task failure handling
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      tasks: {
        failing: {
          description: "This task fails"
          command: "false"
        }
      }
      
      env: {}
      """
    When I run "cuenv task run failing"
    Then the exit code should not be 0
    And the error should contain "failing"