mod bash_integration;
mod fish_integration;
mod hook_installation;
mod interactive_mode;
mod nushell_integration;
mod zsh_integration;

use expectrl::{Eof, Regex, Session};
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::TempDir;

/// Helper to create a shell session with cuenv hooks installed
pub fn create_shell_session(shell: &str, work_dir: &std::path::Path) -> anyhow::Result<Session> {
    let mut session = expectrl::spawn(shell)?;

    // Set up environment
    session.send_line(&format!("cd {}", work_dir.display()))?;

    // Install cuenv hooks
    let hook_cmd = format!("eval \"$(./target/debug/cuenv shell hook {})\"", shell);
    session.send_line(&hook_cmd)?;

    Ok(session)
}

/// Helper to verify environment variable in shell
pub fn check_env_var(
    session: &mut Session,
    var: &str,
    expected: Option<&str>,
) -> anyhow::Result<()> {
    match expected {
        Some(value) => {
            session.send_line(&format!("echo ${}", var))?;
            session.expect(Regex(value))?;
        }
        None => {
            session.send_line(&format!("echo ${{{}-unset}}", var))?;
            session.expect(Regex("unset"))?;
        }
    }
    Ok(())
}

/// Helper to run cuenv in a specific shell
pub fn run_in_shell(shell: &str, commands: &[&str]) -> anyhow::Result<String> {
    let mut child = Command::new(shell)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;

    let stdin = child.stdin.as_mut().unwrap();
    for cmd in commands {
        writeln!(stdin, "{}", cmd)?;
    }
    writeln!(stdin, "exit")?;

    let output = child.wait_with_output()?;
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shell_detection() {
        let shells = vec!["bash", "zsh", "fish", "nu", "dash"];

        for shell in shells {
            // Check if shell is available in PATH
            if Command::new("which")
                .arg(shell)
                .output()
                .unwrap()
                .status
                .success()
            {
                println!("Testing shell: {}", shell);

                let output = Command::new("./target/debug/cuenv")
                    .args(&["shell", "detect"])
                    .env("SHELL", format!("/usr/bin/{}", shell))
                    .output()
                    .expect("Failed to run cuenv shell detect");

                assert!(output.status.success(), "Failed to detect shell: {}", shell);
            }
        }
    }
}
