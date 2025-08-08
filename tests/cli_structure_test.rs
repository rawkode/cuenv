use std::process::Command;
use tempfile::TempDir;

#[test]
fn test_help_shows_all_main_commands() {
    let output = Command::new("./target/debug/cuenv")
        .arg("--help")
        .output()
        .expect("Failed to execute cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // All main commands should be visible
    assert!(stdout.contains("init"), "init command should be visible");
    assert!(
        stdout.contains("status"),
        "status command should be visible"
    );
    assert!(stdout.contains("allow"), "allow command should be visible");
    assert!(stdout.contains("deny"), "deny command should be visible");
    assert!(stdout.contains("run"), "run command should be visible");
    assert!(stdout.contains("exec"), "exec command should be visible");
    assert!(
        stdout.contains("export"),
        "export command should be visible"
    );
    assert!(stdout.contains("dump"), "dump command should be visible");
    assert!(stdout.contains("prune"), "prune command should be visible");
    assert!(stdout.contains("cache"), "cache command should be visible");
    assert!(stdout.contains("shell"), "shell command should be visible");
    assert!(
        stdout.contains("completion"),
        "completion command should be visible"
    );
}

#[test]
fn test_shell_subcommand_exists() {
    let output = Command::new("./target/debug/cuenv")
        .args(["shell", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(output.status.success(), "shell subcommand should exist");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("shell integration") || stdout.contains("Shell"),
        "shell help should show description"
    );
}

#[test]
fn test_shell_init_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["shell", "init", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "shell init subcommand should exist"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("shell hook") || stdout.contains("shell"),
        "shell init help should work"
    );
}

#[test]
fn test_shell_load_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["shell", "load", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "shell load subcommand should exist"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Load environment") || stdout.contains("load"),
        "shell load help should work"
    );
}

#[test]
fn test_shell_unload_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["shell", "unload", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "shell unload subcommand should exist"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Unload") || stdout.contains("unload"),
        "shell unload help should work"
    );
}

#[test]
fn test_shell_hook_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["shell", "hook", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "shell hook subcommand should exist"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Generate shell hook") || stdout.contains("hook"),
        "shell hook help should work"
    );
}

#[test]
fn test_cache_clear_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["cache", "clear", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "cache clear subcommand should exist"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Clear") || stdout.contains("cache"),
        "cache clear help should work"
    );
}

#[test]
fn test_cache_stats_subcommand() {
    let output = Command::new("./target/debug/cuenv")
        .args(["cache", "stats", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        output.status.success(),
        "cache stats subcommand should exist"
    );
}

#[test]
fn test_init_creates_env_cue_file() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");

    let output = Command::new("./target/debug/cuenv")
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute cuenv");

    assert!(output.status.success(), "init command should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created env.cue"),
        "Should report file creation"
    );

    assert!(env_file.exists(), "env.cue should be created");
    let content = std::fs::read_to_string(&env_file).unwrap();
    assert!(
        content.contains("package main"),
        "Should contain package declaration"
    );
    assert!(
        content.contains("import \"cuenv.io/env\""),
        "Should contain import"
    );
    assert!(
        content.contains("environment:"),
        "Should contain environment section"
    );
    assert!(content.contains("tasks:"), "Should contain tasks section");
}

#[test]
fn test_init_refuses_to_overwrite_without_force() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    std::fs::write(&env_file, "existing content").unwrap();

    let output = Command::new("./target/debug/cuenv")
        .current_dir(temp_dir.path())
        .arg("init")
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        !output.status.success(),
        "init should fail when file exists"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("already exists") || stderr.contains("Error"),
        "Should show error about existing file"
    );

    // Content should not be changed
    let content = std::fs::read_to_string(&env_file).unwrap();
    assert_eq!(content, "existing content", "File should not be modified");
}

#[test]
fn test_init_force_overwrites_existing_file() {
    let temp_dir = TempDir::new().unwrap();
    let env_file = temp_dir.path().join("env.cue");
    std::fs::write(&env_file, "existing content").unwrap();

    let output = Command::new("./target/debug/cuenv")
        .current_dir(temp_dir.path())
        .args(["init", "--force"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(output.status.success(), "init --force should succeed");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Created env.cue"),
        "Should report file creation"
    );

    // Content should be replaced
    let content = std::fs::read_to_string(&env_file).unwrap();
    assert!(
        content.contains("package main"),
        "File should be overwritten"
    );
    assert!(
        content.contains("import \"cuenv.io/env\""),
        "File should contain new content"
    );
}

#[test]
fn test_all_commands_have_descriptions() {
    let output = Command::new("./target/debug/cuenv")
        .arg("--help")
        .output()
        .expect("Failed to execute cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Check that each visible command has a description
    assert!(
        stdout.contains("init") && stdout.contains("Initialize"),
        "init should have description"
    );
    assert!(
        stdout.contains("status") && stdout.contains("Display"),
        "status should have description"
    );
    assert!(
        stdout.contains("allow") && stdout.contains("Allow"),
        "allow should have description"
    );
    assert!(
        stdout.contains("deny") && stdout.contains("Deny"),
        "deny should have description"
    );
    assert!(
        stdout.contains("run") && stdout.contains("Run"),
        "run should have description"
    );
    assert!(
        stdout.contains("exec") && stdout.contains("Execute"),
        "exec should have description"
    );
    assert!(
        stdout.contains("export") && stdout.contains("Export"),
        "export should have description"
    );
    assert!(
        stdout.contains("dump") && stdout.contains("Dump"),
        "dump should have description"
    );
    assert!(
        stdout.contains("prune") && stdout.contains("Prune"),
        "prune should have description"
    );
    assert!(
        stdout.contains("cache") && stdout.contains("Manage"),
        "cache should have description"
    );
    assert!(
        stdout.contains("shell") && stdout.contains("Configure"),
        "shell should have description"
    );
    assert!(
        stdout.contains("completion") && stdout.contains("Generate"),
        "completion should have description"
    );
}

#[test]
fn test_legacy_clear_cache_removed() {
    let output = Command::new("./target/debug/cuenv")
        .arg("clear-cache")
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        !output.status.success(),
        "clear-cache command should not exist"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected")
            || stderr.contains("unrecognized")
            || stderr.contains("error"),
        "Should show error for unknown command"
    );
}

#[test]
fn test_load_command_removed() {
    // The load command should no longer exist at the top level
    let output = Command::new("./target/debug/cuenv")
        .args(["load", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        !output.status.success(),
        "load command should not exist at top level"
    );
}

#[test]
fn test_unload_command_removed() {
    // The unload command should no longer exist at the top level
    let output = Command::new("./target/debug/cuenv")
        .args(["unload", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        !output.status.success(),
        "unload command should not exist at top level"
    );
}

#[test]
fn test_hook_command_removed() {
    // The hook command should no longer exist at the top level
    let output = Command::new("./target/debug/cuenv")
        .args(["hook", "--help"])
        .output()
        .expect("Failed to execute cuenv");

    assert!(
        !output.status.success(),
        "hook command should not exist at top level"
    );
}

#[test]
fn test_commands_are_well_organized() {
    let output = Command::new("./target/debug/cuenv")
        .arg("--help")
        .output()
        .expect("Failed to execute cuenv");

    let stdout = String::from_utf8_lossy(&output.stdout);

    // Verify the help text is well-structured
    assert!(stdout.contains("Commands:"), "Should have Commands section");

    // Check for good descriptions for each command
    assert!(
        stdout.contains("Initialize a new env.cue"),
        "init should have full description"
    );
    assert!(
        stdout.contains("Allow cuenv"),
        "allow should have clear description"
    );
    assert!(
        stdout.contains("environment status"),
        "status should describe what it shows"
    );
}
