#![allow(unused)]
use cuenv::config::{CueParser, ParseOptions};
use std::fs;
use tempfile::TempDir;

#[test]
fn test_hooks_integration() {
    let dir = TempDir::new().unwrap();
    let cue_file = dir.path().join("test.cue");

    let content = r#"package env

hooks: {
    onEnter: {
        command: "echo"
        args: ["Starting test environment"]
    }
    onExit: {
        command: "echo"
        args: ["Stopping test environment"]
        url: "https://example.com/stop"
    }
}

env: {
    TEST_VAR: "test_value"
}"#;

    fs::write(&cue_file, content).unwrap();

    let options = ParseOptions::default();
    let result = CueParser::eval_package_with_options(dir.path(), "env", &options).unwrap();

    // Check variables
    assert_eq!(result.variables.get("TEST_VAR").unwrap(), "test_value");

    // Check hooks
    assert_eq!(result.hooks.len(), 2);

    let on_enter = &result.hooks.get("onEnter").unwrap()[0];
    match on_enter {
        cuenv::config::Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "echo");
            assert_eq!(hook_config.args, vec!["Starting test environment"]);
        }
        cuenv::config::Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "echo");
            assert_eq!(
                exec.args.as_ref().unwrap(),
                &vec!["Starting test environment"]
            );
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }

    let on_exit = &result.hooks.get("onExit").unwrap()[0];
    match on_exit {
        cuenv::config::Hook::Legacy(hook_config) => {
            assert_eq!(hook_config.command, "echo");
            assert_eq!(hook_config.args, vec!["Stopping test environment"]);
            assert_eq!(
                hook_config.url,
                Some("https://example.com/stop".to_string())
            );
        }
        cuenv::config::Hook::Exec { exec, .. } => {
            assert_eq!(exec.command, "echo");
            assert_eq!(
                exec.args.as_ref().unwrap(),
                &vec!["Stopping test environment"]
            );
            // TODO: URL support needs to be added to ExecConfig if needed
        }
        _ => panic!("Expected Legacy or Exec hook"),
    }
}
