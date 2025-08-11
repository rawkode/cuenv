use std::collections::HashMap;

/// Parse shell export statements from command output
/// Handles formats like:
/// - export VAR=value
/// - VAR=value
/// - export VAR="quoted value"
/// - export VAR='single quoted'
#[allow(dead_code)]
pub fn parse_shell_exports(output: &str) -> HashMap<String, String> {
    let mut env_vars = HashMap::new();

    for line in output.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        // Handle export statements: export VAR=value or VAR=value
        let export_line = if let Some(stripped) = line.strip_prefix("export ") {
            stripped
        } else {
            line
        };

        // Find the first = to split key=value
        if let Some(eq_pos) = export_line.find('=') {
            let key = export_line[..eq_pos].trim();
            let value = export_line[eq_pos + 1..].trim();

            // Skip invalid variable names
            // Variable names must start with a letter or underscore, followed by alphanumeric or underscore
            if key.is_empty()
                || !key
                    .chars()
                    .next()
                    .is_some_and(|c| c.is_alphabetic() || c == '_')
                || !key.chars().all(|c| c.is_alphanumeric() || c == '_')
            {
                continue;
            }

            // Handle quoted values
            let cleaned_value = if (value.starts_with('"') && value.ends_with('"'))
                || (value.starts_with('\'') && value.ends_with('\''))
            {
                // Remove surrounding quotes
                &value[1..value.len() - 1]
            } else {
                value
            };

            env_vars.insert(key.to_string(), cleaned_value.to_string());
        }
    }

    env_vars
}
