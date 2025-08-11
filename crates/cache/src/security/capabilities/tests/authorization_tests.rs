//! Tests for authorization and permission checking

use crate::security::capabilities::{
    authority::CapabilityAuthority,
    authorization::{AuthorizationResult, CapabilityChecker},
    operations::CacheOperation,
    tokens::Permission,
};
use std::time::Duration;

#[test]
fn test_capability_checking() {
    let authority = CapabilityAuthority::new("test-authority".to_string());
    let mut checker = CapabilityChecker::new(authority);

    let token = checker
        .issue_token(
            "test-user".to_string(),
            [Permission::Read, Permission::Write].into_iter().collect(),
            vec!["cache/*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    // Should allow read operation on allowed key
    let read_op = CacheOperation::Read {
        key: "cache/test".to_string(),
    };
    let result = checker.check_permission(&token, &read_op).unwrap();
    assert_eq!(result, AuthorizationResult::Authorized);

    // Should deny read operation on disallowed key
    let read_op = CacheOperation::Read {
        key: "other/test".to_string(),
    };
    let result = checker.check_permission(&token, &read_op).unwrap();
    assert_eq!(result, AuthorizationResult::KeyAccessDenied);

    // Should deny operation without permission
    let clear_op = CacheOperation::Clear;
    let result = checker.check_permission(&token, &clear_op).unwrap();
    assert_eq!(result, AuthorizationResult::InsufficientPermissions);
}
