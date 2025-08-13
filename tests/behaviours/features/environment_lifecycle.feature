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