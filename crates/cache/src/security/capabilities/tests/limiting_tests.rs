//! Tests for rate limiting functionality

use crate::security::capabilities::{
    authority::CapabilityAuthority,
    authorization::{AuthorizationResult, CapabilityChecker},
    operations::CacheOperation,
    tokens::{Permission, TokenMetadata},
};
use std::time::Duration;

#[test]
fn test_rate_limiting() {
    let authority = CapabilityAuthority::new("test-authority".to_string());
    let mut checker = CapabilityChecker::new(authority);

    let metadata = TokenMetadata {
        rate_limit: Some(2.0), // 2 operations per second
        ..Default::default()
    };

    let token = checker
        .issue_token(
            "test-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(3600),
            Some(metadata),
        )
        .unwrap();

    let read_op = CacheOperation::Read {
        key: "test".to_string(),
    };

    // First two operations should succeed
    assert_eq!(
        checker.check_permission(&token, &read_op).unwrap(),
        AuthorizationResult::Authorized
    );
    assert_eq!(
        checker.check_permission(&token, &read_op).unwrap(),
        AuthorizationResult::Authorized
    );

    // Third operation should be rate limited
    assert_eq!(
        checker.check_permission(&token, &read_op).unwrap(),
        AuthorizationResult::RateLimitExceeded
    );
}
