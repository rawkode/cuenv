//! Tests for token creation and management

use crate::security::capabilities::tokens::{CacheCapability, CapabilityToken, Permission};
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn test_token_creation() {
    let token = CapabilityToken::new(
        "test-user".to_string(),
        vec![CacheCapability::Read, CacheCapability::Write],
        3600,
    );

    assert_eq!(token.subject, "test-user");
    assert!(token.permissions.contains(&Permission::Read));
    assert!(token.permissions.contains(&Permission::Write));
    assert!(token.expires_at > token.issued_at);
    assert!(token.id().starts_with("token_"));
}

#[test]
fn test_capability_to_permission_mapping() {
    let token = CapabilityToken::new(
        "test-user".to_string(),
        vec![
            CacheCapability::Read,
            CacheCapability::Write,
            CacheCapability::Delete,
            CacheCapability::List,
            CacheCapability::Admin,
        ],
        3600,
    );

    assert!(token.permissions.contains(&Permission::Read));
    assert!(token.permissions.contains(&Permission::Write));
    assert!(token.permissions.contains(&Permission::Delete));
    assert!(token.permissions.contains(&Permission::List));
    assert!(token.permissions.contains(&Permission::ManageTokens));
}

#[test]
fn test_token_expiration_time() {
    let validity_seconds = 7200; // 2 hours
    let before = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let token = CapabilityToken::new(
        "test-user".to_string(),
        vec![CacheCapability::Read],
        validity_seconds,
    );

    let after = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();

    // Check that issued_at is set correctly
    assert!(token.issued_at >= before);
    assert!(token.issued_at <= after);

    // Check that expires_at is set correctly
    assert_eq!(token.expires_at, token.issued_at + validity_seconds);
}
