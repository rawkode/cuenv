use expectrl::{Eof, Regex, Session};
use std::time::Duration;
use tempfile::TempDir;

/// Test quit behavior in interactive mode
#[test]
fn test_interactive_quit() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    let cue_content = r#"
package cuenv

hooks: {
    pre: [{
        name: "long-hook"
        command: "sleep"
        args: ["10"]
    }]
}

env: {}
"#;

    std::fs::write(temp_dir.path().join("env.cue"), cue_content)?;

    let mut session = expectrl::spawn("./target/debug/cuenv")?;
    session.send_line(&format!("cd {}", temp_dir.path().display()))?;
    session.send_line("env allow")?;

    // Wait for interactive prompt
    std::thread::sleep(Duration::from_millis(1500));

    // Send 'q' to quit
    session.send("q")?;

    // Verify process terminates
    session.expect(Eof)?;

    Ok(())
}

/// Test Ctrl-C handling in interactive mode
#[test]
fn test_interactive_interrupt() -> anyhow::Result<()> {
    let temp_dir = TempDir::new()?;

    let cue_content = r#"
package cuenv

hooks: {
    pre: [{
        name: "interruptible"
        command: "sleep"
        args: ["10"]
    }]
}

env: {}
"#;

    std::fs::write(temp_dir.path().join("env.cue"), cue_content)?;

    let mut session = expectrl::spawn("./target/debug/cuenv")?;
    session.send_line(&format!("cd {}", temp_dir.path().display()))?;
    session.send_line("env allow")?;

    // Wait for hooks to start
    std::thread::sleep(Duration::from_millis(1500));

    // Send Ctrl-C
    session.send_control('c')?;

    // Verify interruption
    session.expect(Regex("interrupted|terminated"))?;

    Ok(())
}
