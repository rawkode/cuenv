Feature: Environment Lifecycle
  As a developer
  I want to manage environment configurations
  So that I can work with different settings per project

  Background:
    Given I have a temporary test directory

  Scenario: Load simple environment
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        DATABASE_URL: "postgres://localhost/test"
        API_KEY: "test-key"
      }
      """
    When I run "cuenv env export"
    Then the output should contain "DATABASE_URL=postgres://localhost/test"
    And the output should contain "API_KEY=test-key"

  Scenario: Switch between environments
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      environment: {
        development: {
          DATABASE_URL: "postgres://localhost/dev"
        }
        production: {
          DATABASE_URL: "postgres://prod-server/app"
        }
      }
      
      env: environment[currentEnv]
      currentEnv: string | *"development" @tag(env)
      """
    When I run "cuenv env export --env development"
    Then the output should contain "DATABASE_URL=postgres://localhost/dev"
    When I run "cuenv env export --env production"
    Then the output should contain "DATABASE_URL=postgres://prod-server/app"

  Scenario: Inherit from parent directory
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        ROOT_VAR: "root-value"
      }
      """
    And I have a subdirectory "child"
    And I have a CUE file "child/env.cue" with:
      """
      package cuenv
      env: {
        CHILD_VAR: "child-value"
      }
      """
    When I run "cuenv env export" in "child"
    Then the output should contain "ROOT_VAR=root-value"
    And the output should contain "CHILD_VAR=child-value"

  Scenario: Allow environment activation
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        ALLOWED_VAR: "allowed"
      }
      """
    When I run "cuenv env allow"
    Then a ".cuenv/allowed" file should exist
    And the exit code should be 0

  Scenario: Deny environment activation
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        DENIED_VAR: "denied"
      }
      """
    When I run "cuenv env deny"
    Then a ".cuenv/denied" file should exist
    And the exit code should be 0

  Scenario: Environment with capabilities
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        BASIC_VAR: "always-present"
        if capabilities.aws {
          AWS_REGION: "us-east-1"
          AWS_ACCESS_KEY: "key" @capability(aws)
        }
      }
      
      capabilities: {
        aws: bool | *false @tag(aws)
      }
      """
    When I run "cuenv env export"
    Then the output should contain "BASIC_VAR=always-present"
    And the output should not contain "AWS_REGION"
    When I run "cuenv env export --capability aws"
    Then the output should contain "BASIC_VAR=always-present"
    And the output should contain "AWS_REGION=us-east-1"
    And the output should contain "AWS_ACCESS_KEY=key"

  Scenario: Environment unloads when leaving directory
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        TEST_VAR: "test-value"
        CUENV_TEST: "cuenv-specific"
      }
      """
    And I have allowed the current directory
    When I run "cuenv shell load"
    Then the environment should have "TEST_VAR" set to "test-value"
    And the environment should have "CUENV_TEST" set to "cuenv-specific"
    When I change to parent directory
    And I run "cuenv shell hook"
    Then the environment should not have "TEST_VAR"
    And the environment should not have "CUENV_TEST"

  Scenario: Multiple project directory switching maintains isolation
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        PROJECT_NAME: "project1"
        DATABASE_URL: "postgres://localhost/project1"
      }
      """
    And I have allowed the current directory
    And I have a subdirectory "project2"
    And I have a CUE file "project2/env.cue" with:
      """
      package cuenv
      env: {
        PROJECT_NAME: "project2"
        DATABASE_URL: "postgres://localhost/project2"
        API_SECRET: "project2-secret"
      }
      """
    When I run "cuenv shell load"
    Then the environment should have "PROJECT_NAME" set to "project1"
    And the environment should have "DATABASE_URL" set to "postgres://localhost/project1"
    When I change to "project2" directory
    And I run "cuenv shell hook"
    Then the environment should have "PROJECT_NAME" set to "project2"
    And the environment should have "DATABASE_URL" set to "postgres://localhost/project2"
    And the environment should have "API_SECRET" set to "project2-secret"
    When I change to parent directory
    And I run "cuenv shell hook"
    Then the environment should not have "PROJECT_NAME"
    And the environment should not have "DATABASE_URL"
    And the environment should not have "API_SECRET"

  Scenario: Environment load handles invalid CUE gracefully
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        VALID_VAR: "valid-value"
        INVALID_VAR: missing_quotes
      }
      """
    When I run "cuenv env export"
    Then the exit code should not be 0
    And the output should contain "error"

  Scenario: Environment unload is idempotent
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        UNLOAD_TEST: "test-value"
      }
      """
    And I have allowed the current directory
    When I run "cuenv shell load"
    Then the environment should have "UNLOAD_TEST" set to "test-value"
    When I run "cuenv shell unload"
    Then the environment should not have "UNLOAD_TEST"
    When I run "cuenv shell unload"
    Then the exit code should be 0
    And the environment should not have "UNLOAD_TEST"

  Scenario: Rapid directory switching maintains consistency
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        RAPID_VAR: "project-root"
      }
      """
    And I have allowed the current directory
    And I have a subdirectory "subproject"
    And I have a CUE file "subproject/env.cue" with:
      """
      package cuenv
      env: {
        RAPID_VAR: "sub-project"
      }
      """
    When I run "cuenv shell load"
    Then the environment should have "RAPID_VAR" set to "project-root"
    When I change to "subproject" directory
    And I run "cuenv shell hook"
    Then the environment should have "RAPID_VAR" set to "sub-project"
    When I change to parent directory
    And I run "cuenv shell hook"
    Then the environment should not have "RAPID_VAR"
    When I change to "subproject" directory
    And I run "cuenv shell hook"
    Then the environment should have "RAPID_VAR" set to "sub-project"
    When I change to parent directory
    And I run "cuenv shell hook"
    Then the environment should not have "RAPID_VAR"

  Scenario: Environment state persists across multiple load operations
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        PERSISTENT_VAR: "persistent-value"
        SESSION_ID: "session-123"
      }
      """
    And I have allowed the current directory
    When I run "cuenv shell load"
    Then the environment should have "PERSISTENT_VAR" set to "persistent-value"
    And the environment should have "SESSION_ID" set to "session-123"
    When I run "cuenv shell load"
    Then the environment should have "PERSISTENT_VAR" set to "persistent-value"
    And the environment should have "SESSION_ID" set to "session-123"
    When I run "cuenv shell unload"
    Then the environment should not have "PERSISTENT_VAR"
    And the environment should not have "SESSION_ID"

  Scenario: Default loading with preload hooks
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        BACKGROUND_VAR: "background-value"
        API_ENDPOINT: "https://api.test.com"
      }
      hooks: {
        preload: ["echo 'Preload hook starting'", "sleep 1", "echo 'Preload hook finished'"]
      }
      """
    And I have allowed the current directory
    When I run "cuenv shell hook"
    Then the command should complete within 3 seconds
    And the output should contain "export BACKGROUND_VAR=background-value"
    And the output should contain "export API_ENDPOINT=https://api.test.com"
    And the output should contain "Preload hook starting"
    And the output should contain "Preload hook finished"
    When I run "printenv BACKGROUND_VAR"
    Then the output should contain "background-value"
    When I run "printenv API_ENDPOINT"
    Then the output should contain "https://api.test.com"

  Scenario: Preload hooks wait when using cuenv exec
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        EXEC_VAR: "exec-value"
      }
      hooks: {
        preload: ["echo 'Preload hook for exec'", "sleep 1"]
      }
      """
    And I have allowed the current directory
    When I run "cuenv exec printenv EXEC_VAR"
    Then the command should complete within 5 seconds
    And the output should contain "exec-value"

  Scenario: Preload hooks wait when using cuenv task
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        TASK_VAR: "task-value"
      }
      hooks: {
        preload: ["echo 'Preload hook for task'", "sleep 1"]
      }
      tasks: {
        test_task: {
          description: "Test task that prints env var"
          run: "printenv TASK_VAR"
        }
      }
      """
    And I have allowed the current directory
    When I run "cuenv task test_task"
    Then the command should complete within 5 seconds
    And the output should contain "task-value"

  Scenario: Environment variables available only after slow preload hooks finish
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        SLOW_HOOK_VAR: "slow-hook-value"
        API_TOKEN: "secret-token-123"
      }
      hooks: {
        preload: ["echo 'Starting slow preload hook'", "sleep 5", "echo 'Slow preload hook finished'"]
      }
      """
    And I have allowed the current directory
    When I run "cuenv shell hook"
    Then the command should complete within 3 seconds
    And the output should contain "export SLOW_HOOK_VAR=slow-hook-value"
    And the output should contain "export API_TOKEN=secret-token-123"
    When I run "printenv SLOW_HOOK_VAR"
    Then the exit code should not be 0
    When I wait for 7 seconds
    And I run "printenv SLOW_HOOK_VAR"
    Then the exit code should be 0
    And the output should contain "slow-hook-value"
    When I run "printenv API_TOKEN"
    Then the exit code should be 0
    And the output should contain "secret-token-123"