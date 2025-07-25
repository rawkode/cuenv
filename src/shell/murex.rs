use super::Shell;

pub struct MurexShell;

impl Shell for MurexShell {
    fn hook(&self) -> String {
        r#"event onPrompt cuenv {
    cuenv hook murex -> source
}"#
        .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("export {key} = {}", self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("!export {key}")
    }

    fn escape(&self, s: &str) -> String {
        // Murex uses JSON-style string escaping
        let mut result = String::with_capacity(s.len() + 2);
        result.push('"');

        for c in s.chars() {
            match c {
                '"' => result.push_str("\\\""),
                '\\' => result.push_str("\\\\"),
                '\n' => result.push_str("\\n"),
                '\r' => result.push_str("\\r"),
                '\t' => result.push_str("\\t"),
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
    fn test_murex_export() {
        let shell = MurexShell;
        assert_eq!(shell.export("FOO", "bar"), r#"export FOO = "bar""#);
        assert_eq!(shell.export("FOO", "bar baz"), r#"export FOO = "bar baz""#);
    }

    #[test]
    fn test_murex_unset() {
        let shell = MurexShell;
        assert_eq!(shell.unset("FOO"), "!export FOO");
    }

    #[test]
    fn test_murex_escape() {
        let shell = MurexShell;
        assert_eq!(shell.escape("hello"), r#""hello""#);
        assert_eq!(shell.escape(r#"hello "world""#), r#""hello \"world\"""#);
        assert_eq!(shell.escape("line1\nline2"), r#""line1\nline2""#);
    }
}
