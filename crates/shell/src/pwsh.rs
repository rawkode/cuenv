use super::Shell;

pub struct PwshShell;

impl Shell for PwshShell {
    fn hook(&self) -> String {
        r#"$Global:_cuenvOriginalPrompt = $function:prompt
function global:prompt {
    $null = & cuenv hook pwsh | Out-String | Invoke-Expression
    & $Global:_cuenvOriginalPrompt
}"#
        .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!(r#"$env:{} = {}"#, key, self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!(r#"Remove-Item -Path Env:\{key} -ErrorAction SilentlyContinue"#)
    }

    fn escape(&self, s: &str) -> String {
        let mut result = String::with_capacity(s.len() + 10);
        result.push('"');

        for c in s.chars() {
            match c {
                '"' => result.push_str("`\""),
                '`' => result.push_str("``"),
                '$' => result.push_str("`$"),
                _ => result.push(c),
            }
        }

        result.push('"');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pwsh_export() {
        let shell = PwshShell;
        assert_eq!(shell.export("FOO", "bar"), r#"$env:FOO = "bar""#);
        assert_eq!(shell.export("FOO", "bar baz"), r#"$env:FOO = "bar baz""#);
        assert_eq!(shell.export("FOO", "$HOME"), r#"$env:FOO = "`$HOME""#);
    }

    #[test]
    fn test_pwsh_unset() {
        let shell = PwshShell;
        assert_eq!(
            shell.unset("FOO"),
            r#"Remove-Item -Path Env:\FOO -ErrorAction SilentlyContinue"#
        );
    }

    #[test]
    fn test_pwsh_escape() {
        let shell = PwshShell;
        assert_eq!(shell.escape("hello"), r#""hello""#);
        assert_eq!(shell.escape("hello world"), r#""hello world""#);
        assert_eq!(shell.escape(r#"hello "world""#), r#""hello `"world`"""#);
        assert_eq!(shell.escape("$HOME"), r#""`$HOME""#);
        assert_eq!(shell.escape("back`tick"), r#""back``tick""#);
    }

    #[test]
    fn test_pwsh_hook() {
        let shell = PwshShell;
        let hook = shell.hook();
        assert!(hook.contains("_cuenvOriginalPrompt"));
        assert!(hook.contains("function global:prompt"));
    }
}
