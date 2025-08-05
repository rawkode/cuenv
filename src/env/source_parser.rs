use anyhow::{Context, Result};
use std::collections::HashMap;
use std::io::Write;
use std::process::{Command, Stdio};
use tempfile::NamedTempFile;
use tracing::{debug, trace};
use which::which;

/// Evaluate shell script and extract resulting environment variables
/// This executes the shell script output (like from nix print-dev-env) and captures
/// the resulting environment, properly evaluating all variable references.
#[cfg(unix)]
pub fn evaluate_shell_environment(shell_script: &str) -> Result<HashMap<String, String>> {
    debug!(
        "Evaluating shell script to extract environment ({} bytes)",
        shell_script.len()
    );

    // By replacing the initial assignments to `nix_saved_...` variables with
    // assignments to an empty string, we effectively disable the environment
    // restoration that happens at the end of the script. This is more robust
    // than trying to regex-match the restoration command itself.
    let modified_script = shell_script
        .replace(
            "export nix_saved_PATH=\"$PATH\"",
            "export nix_saved_PATH=\"\"",
        )
        .replace(
            "export nix_saved_XDG_DATA_DIRS=\"$XDG_DATA_DIRS\"",
            "export nix_saved_XDG_DATA_DIRS=\"\"",
        );

    // Execute the script using bash process substitution to avoid temp files.
    // This is more performant and cleaner.
    // The script sources the provided content and then prints the environment.
    let bash_path =
        which("bash").context("bash command not found. Please install bash to use shell hooks.")?;
    let mut child = Command::new(bash_path)
        .arg("-c")
        .arg("source /dev/stdin && env -0")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .context("Failed to execute shell script for environment evaluation")?;

    // Write the script to stdin
    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(modified_script.as_bytes())
            .context("Failed to write to stdin")?;
    }

    let output = child
        .wait_with_output()
        .context("Failed to wait for shell script evaluation")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(anyhow::anyhow!(
            "Shell script evaluation failed with status {:?}: {}",
            output.status.code(),
            stderr
        ));
    }

    // Parse the null-separated environment output
    let env_output = String::from_utf8_lossy(&output.stdout);
    let mut env = HashMap::new();

    for line in env_output.split('\0') {
        if line.is_empty() {
            continue;
        }

        if let Some((key, value)) = line.split_once('=') {
            // Skip some problematic variables that can interfere
            if key.starts_with("BASH_FUNC_")
                || key == "PS1"
                || key == "PS2"
                || key == "_"
                || key == "PWD"
                || key == "OLDPWD"
                || key == "SHLVL"
            {
                continue;
            }

            env.insert(key.to_string(), value.to_string());
        }
    }

    debug!(
        "Evaluated shell script and extracted {} environment variables",
        env.len()
    );

    Ok(env)
}

/// Placeholder for Windows shell evaluation.
#[cfg(windows)]
pub fn evaluate_shell_environment(shell_script: &str) -> Result<HashMap<String, String>> {
    debug!(
        "Shell script evaluation is not yet supported on Windows ({} bytes)",
        shell_script.len()
    );
    Err(anyhow::anyhow!(
        "Executing shell scripts to source environment is not yet supported on Windows."
    ))
}

/// Parse shell export statements into environment variables
pub fn parse_shell_exports(output: &str) -> Result<HashMap<String, String>> {
    debug!(
        "Parsing shell exports from {} bytes of output",
        output.len()
    );

    // Convert shell exports to dotenv format
    let dotenv_format = convert_exports_to_dotenv(output)?;

    if dotenv_format.is_empty() {
        debug!("No environment variables found in shell output");
        return Ok(HashMap::new());
    }

    // Write to temp file for dotenv parsing
    let temp =
        NamedTempFile::new().context("Failed to create temporary file for environment parsing")?;

    std::fs::write(&temp, &dotenv_format)
        .context("Failed to write environment to temporary file")?;

    // Parse using dotenv - it loads into process env, doesn't return values
    // So we'll read and parse manually
    let env_content = std::fs::read_to_string(temp.path())
        .context("Failed to read temporary environment file")?;

    let mut env = HashMap::new();
    for line in env_content.lines() {
        if let Some((key, value)) = line.split_once('=') {
            // shell-words should have quoted properly, so unquote
            let value = value.trim_matches('"').trim_matches('\'');
            env.insert(key.to_string(), value.to_string());
        }
    }

    debug!("Parsed {} environment variables", env.len());
    Ok(env)
}

/// Convert shell export statements to dotenv format
fn convert_exports_to_dotenv(shell_output: &str) -> Result<String> {
    let mut result = String::new();
    let mut in_multiline = false;
    let mut current_var = String::new();

    for line in shell_output.lines() {
        trace!("Processing line: {}", line);

        // Skip empty lines and comments
        if line.trim().is_empty() || line.trim_start().starts_with('#') {
            continue;
        }

        // Handle multi-line values
        if in_multiline {
            current_var.push('\n');
            current_var.push_str(line);

            // Check if this line ends the multi-line value
            if ends_multiline_value(line) {
                result.push_str(&current_var);
                result.push('\n');
                current_var.clear();
                in_multiline = false;
            }
            continue;
        }

        // Handle export statements
        if let Some(export) = line.strip_prefix("export ") {
            // Check if this starts a multi-line value
            if starts_multiline_value(export) {
                in_multiline = true;
                current_var = export.to_string();
            } else {
                // Single line export
                result.push_str(export);
                result.push('\n');
            }
        } else if let Some(decl) = line.strip_prefix("declare -x ") {
            // Handle bash declare -x format
            result.push_str(decl);
            result.push('\n');
        } else if line.contains('=') && !line.starts_with(' ') {
            // Already in VAR=value format
            result.push_str(line);
            result.push('\n');
        }
    }

    // Handle any remaining multi-line value
    if !current_var.is_empty() {
        result.push_str(&current_var);
        result.push('\n');
    }

    Ok(result)
}

/// Check if a line starts a multi-line value
fn starts_multiline_value(line: &str) -> bool {
    // Look for opening quotes without closing quotes
    if let Some(eq_pos) = line.find('=') {
        let value_part = &line[eq_pos + 1..];
        let trimmed = value_part.trim();

        // Check for unclosed quotes
        if trimmed.starts_with('"') && !trimmed[1..].contains('"') {
            return true;
        }
        if trimmed.starts_with('\'') && !trimmed[1..].contains('\'') {
            return true;
        }
        if trimmed.starts_with('(') && !trimmed.contains(')') {
            return true;
        }
    }
    false
}

/// Check if a line ends a multi-line value
fn ends_multiline_value(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.ends_with('"') || trimmed.ends_with('\'') || trimmed.ends_with(')')
}

/// Filter environment variables that should not be preserved
pub fn filter_environment(env: HashMap<String, String>) -> HashMap<String, String> {
    env.into_iter()
        .filter(|(key, _)| {
            // Skip temporary nix variables
            !key.starts_with("NIX_BUILD_TOP") &&
            !key.starts_with("__NIX_") &&
            // Skip shell internals
            !key.starts_with("BASH_FUNC_") &&
            !key.starts_with("COMP_WORDBREAKS") &&
            // Keep everything else
            true
        })
        .collect()
}

/// Merge XDG_DATA_DIRS properly (append, don't replace)
pub fn merge_xdg_data_dirs(current: Option<String>, new: Option<String>) -> Option<String> {
    match (current, new) {
        (None, None) => None,
        (Some(c), None) => Some(c),
        (None, Some(n)) => Some(n),
        (Some(current), Some(new)) => {
            let mut dirs = Vec::new();
            let mut seen = std::collections::HashSet::new();

            // Add new dirs first (higher priority)
            for dir in new.split(':') {
                let dir = dir.trim_end_matches('/');
                if !dir.is_empty() && seen.insert(dir.to_string()) {
                    dirs.push(dir.to_string());
                }
            }

            // Add existing dirs
            for dir in current.split(':') {
                let dir = dir.trim_end_matches('/');
                if !dir.is_empty() && seen.insert(dir.to_string()) {
                    dirs.push(dir.to_string());
                }
            }

            if dirs.is_empty() {
                None
            } else {
                Some(dirs.join(":"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_exports() {
        let input = r#"
export PATH="/nix/store/abc/bin:$PATH"
export CARGO_HOME="/home/user/.cargo"
export FOO="bar"
"#;

        let env = parse_shell_exports(input).unwrap();
        assert!(env.contains_key("PATH"));
        assert!(env.contains_key("CARGO_HOME"));
        assert_eq!(env.get("FOO"), Some(&"bar".to_string()));
    }

    #[test]
    fn test_filter_nix_variables() {
        let mut env = HashMap::new();
        env.insert("PATH".to_string(), "/usr/bin".to_string());
        env.insert("NIX_BUILD_TOP".to_string(), "/tmp/nix".to_string());
        env.insert("__NIX_INTERNAL".to_string(), "value".to_string());
        env.insert("CARGO_HOME".to_string(), "/home/.cargo".to_string());

        let filtered = filter_environment(env);
        assert!(filtered.contains_key("PATH"));
        assert!(filtered.contains_key("CARGO_HOME"));
        assert!(!filtered.contains_key("NIX_BUILD_TOP"));
        assert!(!filtered.contains_key("__NIX_INTERNAL"));
    }

    #[test]
    fn test_merge_xdg_data_dirs() {
        let current = Some("/usr/share:/usr/local/share".to_string());
        let new = Some("/nix/store/share:/usr/share".to_string());

        let merged = merge_xdg_data_dirs(current, new).unwrap();
        // New dirs come first, duplicates removed
        assert!(merged.starts_with("/nix/store/share"));
        assert!(merged.contains("/usr/local/share"));
        // /usr/share should only appear once
        assert_eq!(merged.matches("/usr/share").count(), 1);
    }
}
