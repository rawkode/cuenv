#![allow(unused)]
use cuenv::cue_parser::{CueParser, ParseOptions};
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

    let on_enter = result.hooks.get("onEnter").unwrap();
    assert_eq!(on_enter.command, "echo");
    assert_eq!(on_enter.args, vec!["Starting test environment"]);
    assert!(on_enter.url.is_none());

    let on_exit = result.hooks.get("onExit").unwrap();
    assert_eq!(on_exit.command, "echo");
    assert_eq!(on_exit.args, vec!["Stopping test environment"]);
    assert_eq!(on_exit.url, Some("https://example.com/stop".to_string()));
}
