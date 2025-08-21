Feature: Supervisor Lifecycle Management
  As a developer
  I want the supervisor to manage environment lifecycle
  So that long-running processes are properly managed

  Background:
    Given I have a temporary test directory

  Scenario: Supervisor starts with environment
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      env: {
        SUPERVISOR_TEST: "active"
      }
      """
    When I run "cuenv env allow"
    Then the supervisor should be running
    And the environment variable "SUPERVISOR_TEST" should equal "active"

  Scenario: Supervisor manages hook lifecycle
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      hooks: {
        pre: [{
          name: "setup"
          command: "echo"
          args: ["Setting up environment"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    Then the supervisor should be running
    And the pre hook "setup" should complete
    When I run "cuenv env deny"
    Then the supervisor should stop

  Scenario: Supervisor handles interactive control
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
        interactive: true
      }
      
      hooks: {
        pre: [{
          name: "slow-hook"
          command: "sleep"
          args: ["10"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow" interactively
    And I wait 1 second
    And I press "q" to quit
    Then the hooks should be terminated
    And control should return immediately

  Scenario: Supervisor tracks hook status
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      hooks: {
        pre: [
          {
            name: "setup-db"
            command: "echo"
            args: ["Setting up database"]
          },
          {
            name: "install-deps"
            command: "echo"
            args: ["Installing dependencies"]
          }
        ]
      }
      
      env: {}
      """
    When I run "cuenv env status"
    Then the output should show hook progress
    And completed hooks should be marked as done

  Scenario: Supervisor handles environment switching
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      environment: {
        dev: {
          hooks: {
            pre: [{
              name: "dev-setup"
              command: "echo"
              args: ["Dev environment"]
            }]
          }
        }
        prod: {
          hooks: {
            pre: [{
              name: "prod-setup"
              command: "echo"
              args: ["Prod environment"]
            }]
          }
        }
      }
      
      env: environment[currentEnv].env // {}
      hooks: environment[currentEnv].hooks // {}
      currentEnv: string | *"dev" @tag(env)
      """
    When I run "cuenv env allow --env dev"
    Then the pre hook "dev-setup" should complete
    When I run "cuenv env allow --env prod"
    Then the pre hook "prod-setup" should complete

  Scenario: Supervisor cleanup on exit
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      hooks: {
        pre: [{
          name: "setup"
          command: "sh"
          args: ["-c", "trap 'echo cleanup' EXIT; sleep 1"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    And the supervisor is running
    When I send SIGTERM to supervisor
    Then the output should contain "cleanup"
    And all processes should be terminated