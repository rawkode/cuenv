use super::{escape_bash_like, Shell};

pub struct TcshShell;

impl Shell for TcshShell {
    fn hook(&self) -> String {
        r#"alias precmd 'eval `cuenv hook tcsh`'"#.to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("setenv {} {}", key, self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("unsetenv {key}")
    }

    fn escape(&self, s: &str) -> String {
        escape_bash_like(s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tcsh_export() {
        let shell = TcshShell;
        assert_eq!(shell.export("FOO", "bar"), "setenv FOO bar");
        assert_eq!(shell.export("FOO", "bar baz"), "setenv FOO 'bar baz'");
    }

    #[test]
    fn test_tcsh_unset() {
        let shell = TcshShell;
        assert_eq!(shell.unset("FOO"), "unsetenv FOO");
    }
}
