use crate::manager::{helpers::parse_shell_exports, AccessRestrictions};

#[test]
fn test_access_restrictions_creation_and_methods() {
    // Test default (no restrictions)
    let restrictions = AccessRestrictions::default();
    assert!(!restrictions.has_any_restrictions());

    // Test with all restrictions
    let restrictions = AccessRestrictions::new(true, true);
    assert!(restrictions.has_any_restrictions());

    // Test with partial restrictions
    let restrictions = AccessRestrictions::new(true, false);
    assert!(restrictions.has_any_restrictions());

    let restrictions = AccessRestrictions::new(false, true);
    assert!(restrictions.has_any_restrictions());

    let restrictions = AccessRestrictions::new(false, false);
    assert!(!restrictions.has_any_restrictions());
}

#[test]
fn test_parse_shell_exports() {
    // Test basic export statements
    let output = r#"
export PATH=/usr/bin:/bin
export HOME=/home/user
DB_URL=postgres://localhost/test
export API_KEY="secret-key"
export TOKEN='bearer-token'
# This is a comment
export EMPTY_VAR=
INVALID_VAR
export =invalid
export 123INVALID=value
        "#;

    let vars = parse_shell_exports(output);

    assert_eq!(vars.get("PATH"), Some(&"/usr/bin:/bin".to_string()));
    assert_eq!(vars.get("HOME"), Some(&"/home/user".to_string()));
    assert_eq!(
        vars.get("DB_URL"),
        Some(&"postgres://localhost/test".to_string())
    );
    assert_eq!(vars.get("API_KEY"), Some(&"secret-key".to_string()));
    assert_eq!(vars.get("TOKEN"), Some(&"bearer-token".to_string()));
    assert_eq!(vars.get("EMPTY_VAR"), Some(&"".to_string()));

    // Invalid variables should not be included
    assert!(!vars.contains_key("INVALID_VAR"));
    assert!(!vars.contains_key(""));
    assert!(!vars.contains_key("123INVALID"));
}