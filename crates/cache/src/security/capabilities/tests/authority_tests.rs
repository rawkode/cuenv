//! Tests for capability authority functionality

use crate::security::capabilities::{
    authority::CapabilityAuthority, tokens::Permission, verification::TokenVerificationResult,
};
use std::collections::HashSet;
use std::thread;
use std::time::Duration;

#[test]
fn test_token_issue_and_verify() {
    let mut authority = CapabilityAuthority::new("test-authority".to_string());

    let mut permissions = HashSet::new();
    permissions.insert(Permission::Read);
    permissions.insert(Permission::Write);

    let token = authority
        .issue_token(
            "test-user".to_string(),
            permissions,
            vec!["test/*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    let result = authority.verify_token(&token).unwrap();
    assert_eq!(result, TokenVerificationResult::Valid);
}

#[test]
fn test_token_expiration() {
    let mut authority = CapabilityAuthority::new("test-authority".to_string());

    let token = authority
        .issue_token(
            "test-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(1), // Use seconds for reliable expiration
            None,
        )
        .unwrap();

    // Wait for expiration
    thread::sleep(Duration::from_secs(2));

    let result = authority.verify_token(&token).unwrap();
    assert_eq!(result, TokenVerificationResult::Expired);
}

#[test]
fn test_token_revocation() {
    let mut authority = CapabilityAuthority::new("test-authority".to_string());

    let token = authority
        .issue_token(
            "test-user".to_string(),
            [Permission::Read].into_iter().collect(),
            vec!["*".to_string()],
            Duration::from_secs(3600),
            None,
        )
        .unwrap();

    // Verify initially valid
    assert_eq!(
        authority.verify_token(&token).unwrap(),
        TokenVerificationResult::Valid
    );

    // Revoke token
    assert!(authority.revoke_token(&token.token_id).unwrap());

    // Should now be revoked
    assert_eq!(
        authority.verify_token(&token).unwrap(),
        TokenVerificationResult::Revoked
    );
}
