use super::Shell;

pub struct ElvishShell;

impl Shell for ElvishShell {
    fn hook(&self) -> String {
        r#"set @before-chdir = {|$@args|
  try {
    $@args
  } finally {
    eval (cuenv hook elvish | slurp)
  }
}"#
        .to_string()
    }

    fn export(&self, key: &str, value: &str) -> String {
        format!("set E:{key} = {}", self.escape(value))
    }

    fn unset(&self, key: &str) -> String {
        format!("unset-env {key}")
    }

    fn escape(&self, s: &str) -> String {
        // Elvish uses single quotes for literal strings
        if s.is_empty() {
            return "''".to_string();
        }

        let mut result = String::with_capacity(s.len() + 2);
        result.push('\'');

        for c in s.chars() {
            if c == '\'' {
                result.push_str("''");
            } else {
                result.push(c);
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
    fn test_elvish_export() {
        let shell = ElvishShell;
        assert_eq!(shell.export("FOO", "bar"), "set E:FOO = 'bar'");
        assert_eq!(shell.export("FOO", "bar baz"), "set E:FOO = 'bar baz'");
    }

    #[test]
    fn test_elvish_unset() {
        let shell = ElvishShell;
        assert_eq!(shell.unset("FOO"), "unset-env FOO");
    }

    #[test]
    fn test_elvish_escape() {
        let shell = ElvishShell;
        assert_eq!(shell.escape("hello"), "'hello'");
        assert_eq!(shell.escape("it's"), "'it''s'");
    }
}
