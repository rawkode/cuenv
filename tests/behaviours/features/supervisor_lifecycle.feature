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

  Scenario: Supervisor manages background hooks
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      hooks: {
        background: [{
          name: "watcher"
          command: "sh"
          args: ["-c", "while true; do echo 'watching'; sleep 1; done"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    Then the supervisor should be running
    And the background hook "watcher" should be running
    When I run "cuenv env deny"
    Then the supervisor should stop
    And the background hook "watcher" should be terminated

  Scenario: Supervisor handles interactive control
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
        interactive: true
      }
      
      hooks: {
        background: [{
          name: "slow-hook"
          command: "sleep"
          args: ["10"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow" interactively
    And I wait 1 second
    And I press "b" to background
    Then the hooks should continue in background
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
            background: [{
              name: "dev-watcher"
              command: "echo"
              args: ["Dev environment"]
            }]
          }
        }
        prod: {
          hooks: {
            background: [{
              name: "prod-monitor"
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
    Then the background hook "dev-watcher" should be running
    When I run "cuenv env allow --env prod"
    Then the background hook "dev-watcher" should be terminated
    And the background hook "prod-monitor" should be running

  Scenario: Supervisor cleanup on exit
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      supervisor: {
        enabled: true
      }
      
      hooks: {
        background: [{
          name: "persistent"
          command: "sh"
          args: ["-c", "trap 'echo cleanup' EXIT; while true; do sleep 1; done"]
        }]
      }
      
      env: {}
      """
    When I run "cuenv env allow"
    And the supervisor is running
    When I send SIGTERM to supervisor
    Then the output should contain "cleanup"
    And all background processes should be terminated