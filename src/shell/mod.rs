use std::collections::HashMap;
use std::path::Path;

pub mod bash;
pub mod cmd;
pub mod elvish;
pub mod fish;
pub mod murex;
pub mod pwsh;
pub mod tcsh;
pub mod zsh;

#[derive(Debug, Clone, PartialEq)]
pub enum ShellType {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Cmd,
    Elvish,
    Tcsh,
    Murex,
    Unknown(String),
}

pub trait Shell {
    fn hook(&self) -> String;

    fn export(&self, key: &str, value: &str) -> String;

    fn unset(&self, key: &str) -> String;

    fn dump(&self, env: &HashMap<String, String>) -> String {
        env.iter()
            .map(|(k, v)| self.export(k, v))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn escape(&self, s: &str) -> String;
}

impl ShellType {
    pub fn detect_from_arg(arg0: &str) -> Self {
        let shell_name = Path::new(arg0)
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or(arg0);

        let shell_name = shell_name.strip_prefix('-').unwrap_or(shell_name);

        Self::from_name(shell_name)
    }

    pub fn from_name(name: &str) -> Self {
        match name {
            "bash" => ShellType::Bash,
            "zsh" => ShellType::Zsh,
            "fish" => ShellType::Fish,
            "pwsh" | "powershell" => ShellType::PowerShell,
            "cmd" | "cmd.exe" => ShellType::Cmd,
            "elvish" => ShellType::Elvish,
            "tcsh" => ShellType::Tcsh,
            "murex" => ShellType::Murex,
            _ => ShellType::Unknown(name.to_string()),
        }
    }

    pub fn as_shell(&self) -> Box<dyn Shell> {
        match self {
            ShellType::Bash => Box::new(bash::BashShell),
            ShellType::Zsh => Box::new(zsh::ZshShell),
            ShellType::Fish => Box::new(fish::FishShell),
            ShellType::PowerShell => Box::new(pwsh::PwshShell),
            ShellType::Cmd => Box::new(cmd::CmdShell),
            ShellType::Elvish => Box::new(elvish::ElvishShell),
            ShellType::Tcsh => Box::new(tcsh::TcshShell),
            ShellType::Murex => Box::new(murex::MurexShell),
            ShellType::Unknown(_) => Box::new(bash::BashShell),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            ShellType::Bash => "bash",
            ShellType::Zsh => "zsh",
            ShellType::Fish => "fish",
            ShellType::PowerShell => "pwsh",
            ShellType::Cmd => "cmd",
            ShellType::Elvish => "elvish",
            ShellType::Tcsh => "tcsh",
            ShellType::Murex => "murex",
            ShellType::Unknown(name) => name,
        }
    }
}

pub fn escape_bash_like(s: &str) -> String {
    if s.is_empty() {
        return "''".to_string();
    }

    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '=' || c == '/' || c == '.')
    {
        return s.to_string();
    }

    let mut result = String::with_capacity(s.len() + 10);
    result.push('\'');

    for c in s.chars() {
        if c == '\'' {
            result.push_str("'\"'\"'");
        } else {
            result.push(c);
        }
    }

    result.push('\'');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_from_arg() {
        assert_eq!(ShellType::detect_from_arg("bash"), ShellType::Bash);
        assert_eq!(ShellType::detect_from_arg("-bash"), ShellType::Bash);
        assert_eq!(ShellType::detect_from_arg("/bin/bash"), ShellType::Bash);
        assert_eq!(ShellType::detect_from_arg("/usr/bin/zsh"), ShellType::Zsh);
        assert_eq!(ShellType::detect_from_arg("-zsh"), ShellType::Zsh);
        assert_eq!(ShellType::detect_from_arg("fish"), ShellType::Fish);
        assert_eq!(ShellType::detect_from_arg("pwsh"), ShellType::PowerShell);
        assert_eq!(
            ShellType::detect_from_arg("unknown"),
            ShellType::Unknown("unknown".to_string())
        );
    }

    #[test]
    fn test_escape_bash_like() {
        assert_eq!(escape_bash_like(""), "''");
        assert_eq!(escape_bash_like("hello"), "hello");
        assert_eq!(escape_bash_like("hello world"), "'hello world'");
        assert_eq!(escape_bash_like("it's"), "'it'\"'\"'s'");
        assert_eq!(escape_bash_like("$HOME"), "'$HOME'");
    }
}
