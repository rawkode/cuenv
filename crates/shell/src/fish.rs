use super::Shell;

pub struct FishShell;

impl Shell for FishShell {
    fn hook(&self) -> String {
        r#"function _cuenv_hook --on-variable PWD --description 'cuenv hook'
  set -l prev_status $status
  cuenv hook fish | source
  if test $prev_status -ne 0
    return $prev_status
  end
end

# Trigger the hook for the initial directory
_cuenv_hook"#
            .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("set -gx {key} {}", self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("set -e {key}")
    }

    fn escape(&self, s: &str) -> String {
        if s.is_empty() {
            return "''".to_string();
        }

        let needs_quotes = s.chars().any(|c| {
            matches!(
                c,
                ' ' | '\t'
                    | '\n'
                    | '\r'
                    | '$'
                    | '&'
                    | '|'
                    | '<'
                    | '>'
                    | '('
                    | ')'
                    | '{'
                    | '}'
                    | '['
                    | ']'
                    | '*'
                    | '?'
                    | ';'
                    | '"'
                    | '\''
                    | '\\'
            )
        });

        if !needs_quotes {
            return s.to_string();
        }

        let mut result = String::with_capacity(s.len() + 2);
        result.push('\'');

        for c in s.chars() {
            match c {
                '\'' => result.push_str("\'\\\'\'"),
                '\\' => result.push_str("\\\\"),
                _ => result.push(c),
            }
        }

        result.push('\'');
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fish_export() {
        let shell = FishShell;
        assert_eq!(shell.export("FOO", "bar"), "set -gx FOO bar");
        assert_eq!(shell.export("FOO", "bar baz"), "set -gx FOO 'bar baz'");
        assert_eq!(shell.export("PATH", "/usr/bin"), "set -gx PATH /usr/bin");
    }

    #[test]
    fn test_fish_unset() {
        let shell = FishShell;
        assert_eq!(shell.unset("FOO"), "set -e FOO");
    }

    #[test]
    fn test_fish_escape() {
        let shell = FishShell;
        assert_eq!(shell.escape("hello"), "hello");
        assert_eq!(shell.escape("hello world"), "'hello world'");
        assert_eq!(shell.escape("it's"), "'it'\\''s'");
        assert_eq!(shell.escape("$HOME"), "'$HOME'");
    }

    #[test]
    fn test_fish_hook() {
        let shell = FishShell;
        let hook = shell.hook();
        assert!(hook.contains("_cuenv_hook"));
        assert!(hook.contains("--on-variable PWD"));
    }
}
