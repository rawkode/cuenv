Feature: Shell Integration
  As a developer
  I want cuenv to integrate with my shell
  So that my environment is automatically configured

  Background:
    Given I have a temporary test directory
    And cuenv is installed

  Scenario: Detect current shell
    When I run "cuenv shell detect"
    Then the output should contain one of:
      | bash |
      | zsh  |
      | fish |
      | nu   |

  Scenario: Install shell hooks for bash
    Given I am using "bash" shell
    When I run "cuenv shell hook bash"
    Then the output should contain "cuenv_hook"
    And the output should contain "export"
    And the output should contain "CUENV_"

  Scenario: Install shell hooks for zsh
    Given I am using "zsh" shell
    When I run "cuenv shell hook zsh"
    Then the output should contain "cuenv_hook"
    And the output should contain "export"
    And the output should contain "precmd"

  Scenario: Install shell hooks for fish
    Given I am using "fish" shell
    When I run "cuenv shell hook fish"
    Then the output should contain "function cuenv_hook"
    And the output should contain "set -x"

  Scenario: Install shell hooks for nushell
    Given I am using "nu" shell
    When I run "cuenv shell hook nu"
    Then the output should contain "def-env cuenv_hook"
    And the output should contain "load-env"

  Scenario: Shell hook activates environment on directory change
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        TEST_VAR: "activated"
      }
      """
    And I am using "bash" shell
    And shell hooks are installed
    When I change to the test directory
    Then the environment variable "TEST_VAR" should equal "activated"

  Scenario: Shell hook deactivates environment on leaving directory
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      env: {
        TEST_VAR: "active"
      }
      """
    And I am using "bash" shell
    And shell hooks are installed
    And I am in the test directory
    When I change to parent directory
    Then the environment variable "TEST_VAR" should not exist