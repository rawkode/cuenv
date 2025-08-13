Feature: Hook Execution
  As a developer
  I want to run hooks when environments change
  So that I can automate setup and teardown tasks

  Background:
    Given I have a temporary test directory

  Scenario: Execute pre-hook before loading environment
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        pre: [{
          name: "setup"
          command: "echo"
          args: ["Pre-hook executed"]
        }]
      }
      
      env: {
        TEST_VAR: "loaded"
      }
      """
    When I run "cuenv env allow"
    Then the output should contain "Pre-hook executed"
    And the output should contain "setup"

  Scenario: Execute post-hook after loading environment
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        post: [{
          name: "verify"
          command: "echo"
          args: ["Post-hook executed"]
        }]
      }
      
      env: {
        TEST_VAR: "loaded"
      }
      """
    When I run "cuenv env allow"
    Then the output should contain "Post-hook executed"
    And the output should contain "verify"

  Scenario: Execute hooks in sequence
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        pre: [
          {
            name: "first"
            command: "echo"
            args: ["First hook"]
          },
          {
            name: "second"
            command: "echo"
            args: ["Second hook"]
          }
        ]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    Then the output should match pattern "First hook.*Second hook"

  Scenario: Background hooks with progress indicator
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        background: [{
          name: "long-running"
          command: "sleep"
          args: ["2"]
        }]
      }
      
      env: {
        TEST_VAR: "loaded"
      }
      """
    When I run "cuenv env allow" with timeout 5 seconds
    Then the hook should complete within 3 seconds
    And the exit code should be 0

  Scenario: Hook with environment variables
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        pre: [{
          name: "env-check"
          command: "sh"
          args: ["-c", "echo $HOOK_ENV_VAR"]
          env: {
            HOOK_ENV_VAR: "custom-value"
          }
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    Then the output should contain "custom-value"

  Scenario: Conditional hook execution
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        pre: [{
          name: "conditional"
          command: "echo"
          args: ["Hook executed"]
          if: environment == "development"
        }]
      }
      
      environment: string | *"development" @tag(env)
      env: {}
      """
    When I run "cuenv env allow --env development"
    Then the output should contain "Hook executed"
    When I run "cuenv env allow --env production"
    Then the output should not contain "Hook executed"

  Scenario: Hook failure handling
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      hooks: {
        pre: [{
          name: "failing-hook"
          command: "false"
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    Then the exit code should not be 0
    And the error should contain "failing-hook"