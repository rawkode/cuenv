Feature: Secret Resolution
  As a developer
  I want to resolve secrets dynamically
  So that sensitive data is not stored in configuration files

  Background:
    Given I have a temporary test directory

  Scenario: Resolve secret using command
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        DATABASE_PASSWORD: {
          resolver: {
            command: "echo"
            args: ["super-secret-password"]
          }
        }
      }
      """
    When I run "cuenv env export"
    Then the output should contain "DATABASE_PASSWORD=super-secret-password"

  Scenario: Resolve secret from file
    Given I have a file "secret.txt" with content "file-secret-value"
    And I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        API_KEY: {
          resolver: {
            command: "cat"
            args: ["secret.txt"]
          }
        }
      }
      """
    When I run "cuenv env export"
    Then the output should contain "API_KEY=file-secret-value"

  Scenario: Resolve secret with environment context
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        AWS_SECRET: {
          resolver: {
            command: "sh"
            args: ["-c", "echo secret-for-$ENV_NAME"]
            env: {
              ENV_NAME: environment
            }
          }
        }
      }
      
      environment: string | *"development" @tag(env)
      """
    When I run "cuenv env export --env production"
    Then the output should contain "AWS_SECRET=secret-for-production"

  Scenario: Cache resolved secrets
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        CACHED_SECRET: {
          resolver: {
            command: "sh"
            args: ["-c", "echo $(date +%s)-secret"]
            cache: {
              ttl: 60
            }
          }
        }
      }
      """
    When I run "cuenv env export"
    Then I remember the value of "CACHED_SECRET"
    When I run "cuenv env export" again
    Then the value of "CACHED_SECRET" should be the same

  Scenario: Obfuscate secrets in output
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        PUBLIC_VAR: "visible-value"
        SECRET_VAR: {
          resolver: {
            command: "echo"
            args: ["hidden-secret"]
          }
          sensitive: true
        }
      }
      """
    When I run "cuenv env show"
    Then the output should contain "PUBLIC_VAR=visible-value"
    And the output should contain "SECRET_VAR=***"
    And the output should not contain "hidden-secret"

  Scenario: Resolver with custom protocol
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      secrets: {
        resolvers: {
          vault: {
            command: "vault"
            args: ["kv", "get", "-field=password"]
          }
        }
      }
      
      env: {
        DB_PASSWORD: {
          resolver: "vault"
          path: "secret/database"
        }
      }
      """
    When I mock "vault" to return "vault-secret"
    And I run "cuenv env export"
    Then the output should contain "DB_PASSWORD=vault-secret"

  Scenario: Handle resolver failure
    Given I have a CUE file "env.cue" with:
      """
      package cuenv
      
      env: {
        FAILING_SECRET: {
          resolver: {
            command: "false"
          }
        }
      }
      """
    When I run "cuenv env export"
    Then the exit code should not be 0
    And the error should contain "FAILING_SECRET"
    And the error should contain "resolver failed"