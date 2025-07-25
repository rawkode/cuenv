use super::{escape_bash_like, Shell};

pub struct ZshShell;

impl Shell for ZshShell {
    fn hook(&self) -> String {
        r#"_cuenv_hook() {
  trap -- '' SIGINT
  eval "$(cuenv hook zsh)"
  trap - SIGINT
}
typeset -ag precmd_functions
if [[ ${precmd_functions[(ie)_cuenv_hook]} -gt ${#precmd_functions} ]]; then
  precmd_functions+=(_cuenv_hook)
fi"#
        .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("export {}={}", key, self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("unset {}", key)
    }

    fn escape(&self, s: &str) -> String {
        escape_bash_like(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zsh_export() {
        let shell = ZshShell;
        assert_eq!(shell.export("FOO", "bar"), "export FOO=bar");
        assert_eq!(shell.export("FOO", "bar baz"), "export FOO='bar baz'");
    }

    #[test]
    fn test_zsh_unset() {
        let shell = ZshShell;
        assert_eq!(shell.unset("FOO"), "unset FOO");
    }

    #[test]
    fn test_zsh_hook() {
        let shell = ZshShell;
        let hook = shell.hook();
        assert!(hook.contains("_cuenv_hook"));
        assert!(hook.contains("precmd_functions"));
    }
}
