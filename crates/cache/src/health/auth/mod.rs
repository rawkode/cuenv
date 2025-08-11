//! Authentication and authorization for health endpoints
//!
//! Provides token-based authentication for protecting sensitive endpoints
//! with support for expiration and endpoint-specific permissions.

use hyper::Request;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;
use tokio::sync::RwLock;

/// Authentication token information
#[derive(Debug, Clone)]
pub struct AuthToken {
    /// Token name/description
    pub name: String,
    /// Token creation time
    pub created_at: SystemTime,
    /// Optional expiration time
    pub expires_at: Option<SystemTime>,
    /// Allowed endpoints for this token
    pub allowed_endpoints: Vec<String>,
}

/// Authentication manager for health endpoints
#[derive(Debug)]
pub struct AuthManager {
    /// Stored authentication tokens
    tokens: Arc<RwLock<HashMap<String, AuthToken>>>,
    /// Whether authentication is required
    require_auth: bool,
}

impl AuthManager {
    /// Create a new authentication manager
    pub fn new(require_auth: bool) -> Self {
        Self {
            tokens: Arc::new(RwLock::new(HashMap::new())),
            require_auth,
        }
    }

    /// Add an authentication token
    pub async fn add_token(
        &self,
        token: String,
        name: String,
        expires_at: Option<SystemTime>,
        allowed_endpoints: Vec<String>,
    ) {
        let auth_token = AuthToken {
            name,
            created_at: SystemTime::now(),
            expires_at,
            allowed_endpoints,
        };

        self.tokens.write().await.insert(token, auth_token);
    }

    /// Validate authentication token for a specific endpoint
    pub async fn validate<B>(&self, req: &Request<B>, endpoint: &str) -> Result<(), ()> {
        if !self.require_auth {
            return Ok(());
        }

        // Extract token from Authorization header
        let token = match req.headers().get("Authorization") {
            Some(auth_header) => {
                let auth_str = match auth_header.to_str() {
                    Ok(s) => s,
                    Err(_) => return Err(()),
                };

                auth_str.strip_prefix("Bearer ").ok_or(())?
            }
            None => return Err(()),
        };

        let tokens = self.tokens.read().await;
        match tokens.get(token) {
            Some(auth_token) => {
                // Check if token is expired
                if let Some(expires_at) = auth_token.expires_at {
                    if SystemTime::now() > expires_at {
                        return Err(());
                    }
                }

                // Check if token is allowed for this endpoint
                if !auth_token.allowed_endpoints.is_empty()
                    && !auth_token
                        .allowed_endpoints
                        .iter()
                        .any(|allowed| endpoint.starts_with(allowed) || allowed == "*")
                {
                    return Err(());
                }

                Ok(())
            }
            None => Err(()),
        }
    }

    /// Get reference to internal tokens for cloning
    pub fn tokens(&self) -> Arc<RwLock<HashMap<String, AuthToken>>> {
        Arc::clone(&self.tokens)
    }
}
