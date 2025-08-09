use super::Shell;

pub struct CmdShell;

impl Shell for CmdShell {
    fn hook(&self) -> String {
        // CMD doesn't support automatic hooks, provide manual function
        r#":: cuenv hook for cmd.exe
:: Call _cuenv_hook manually when changing directories
doskey _cuenv_hook=FOR /F "tokens=*" %i IN ('cuenv hook cmd') DO %i"#
            .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("set {key}={}", self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("set {key}=")
    }

    fn escape(&self, s: &str) -> String {
        // CMD has limited escaping capabilities
        // Just handle basic cases
        if s.contains(' ')
            || s.contains('&')
            || s.contains('|')
            || s.contains('>')
            || s.contains('<')
        {
            format!("\"{}\"", s.replace('"', "\"\""))
        } else {
            s.to_string()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cmd_export() {
        let shell = CmdShell;
        assert_eq!(shell.export("FOO", "bar"), "set FOO=bar");
        assert_eq!(shell.export("FOO", "bar baz"), "set FOO=\"bar baz\"");
    }

    #[test]
    fn test_cmd_unset() {
        let shell = CmdShell;
        assert_eq!(shell.unset("FOO"), "set FOO=");
    }

    #[test]
    fn test_cmd_escape() {
        let shell = CmdShell;
        assert_eq!(shell.escape("hello"), "hello");
        assert_eq!(shell.escape("hello world"), "\"hello world\"");
        assert_eq!(shell.escape("hello&world"), "\"hello&world\"");
    }

    #[test]
    fn test_cmd_hook() {
        let shell = CmdShell;
        let hook = shell.hook();
        assert!(hook.contains("_cuenv_hook"));
        assert!(hook.contains("doskey"));
    }
}
