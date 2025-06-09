use anyhow::{Context, Result};
use async_trait::async_trait;
use lazy_static::lazy_static;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::process::Command;

lazy_static! {
    // Matches op://vault/item/field or op://vault/item
    static ref ONEPASSWORD_REGEX: Regex = Regex::new(r"op://([^/]+)/([^/]+)(?:/([^/]+))?").unwrap();
    
    // Matches gcp-secret://project/secret/version or gcp-secret://project/secret
    static ref GCP_SECRET_REGEX: Regex = Regex::new(r"gcp-secret://([^/]+)/([^/]+)(?:/([^/]+))?").unwrap();
}

#[async_trait]
pub trait SecretResolver: Send + Sync {
    async fn resolve(&self, reference: &str) -> Result<Option<String>>;
}

pub struct OnePasswordResolver;

impl OnePasswordResolver {
    pub fn new() -> Self {
        Self
    }
    
    fn run_op_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("op")
            .args(args)
            .output()
            .context("Failed to execute 'op' command. Is 1Password CLI installed?")?;
            
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("1Password CLI error: {}", stderr);
        }
        
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }
}

#[async_trait]
impl SecretResolver for OnePasswordResolver {
    async fn resolve(&self, reference: &str) -> Result<Option<String>> {
        if let Some(captures) = ONEPASSWORD_REGEX.captures(reference) {
            let vault = captures.get(1).unwrap().as_str();
            let item = captures.get(2).unwrap().as_str();
            let field = captures.get(3).map(|m| m.as_str());
            
            let result = match field {
                Some(f) => {
                    let uri = format!("op://{}/{}/{}", vault, item, f);
                    self.run_op_command(&["read", &uri])?
                }
                None => {
                    self.run_op_command(&["item", "get", item, "--vault", vault, "--format", "json"])?
                }
            };
            
            // If no field was specified, we got JSON and need to extract the password
            if field.is_none() {
                let json: serde_json::Value = serde_json::from_str(&result)?;
                if let Some(fields) = json["fields"].as_array() {
                    for field in fields {
                        if field["purpose"].as_str() == Some("PASSWORD") {
                            if let Some(value) = field["value"].as_str() {
                                return Ok(Some(value.to_string()));
                            }
                        }
                    }
                }
            } else {
                return Ok(Some(result));
            }
        }
        
        Ok(None)
    }
}

pub struct GcpSecretResolver;

impl GcpSecretResolver {
    pub fn new() -> Self {
        Self
    }
    
    fn run_gcloud_command(&self, args: &[&str]) -> Result<String> {
        let output = Command::new("gcloud")
            .args(args)
            .output()
            .context("Failed to execute 'gcloud' command. Is Google Cloud SDK installed?")?;
            
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!("gcloud CLI error: {}", stderr);
        }
        
        Ok(String::from_utf8(output.stdout)?.trim().to_string())
    }
}

#[async_trait]
impl SecretResolver for GcpSecretResolver {
    async fn resolve(&self, reference: &str) -> Result<Option<String>> {
        if let Some(captures) = GCP_SECRET_REGEX.captures(reference) {
            let project = captures.get(1).unwrap().as_str();
            let secret = captures.get(2).unwrap().as_str();
            let version = captures.get(3).map(|m| m.as_str()).unwrap_or("latest");
            
            let args = vec![
                "secrets", "versions", "access", version,
                "--secret", secret,
                "--project", project,
            ];
            
            let result = self.run_gcloud_command(&args)?;
            return Ok(Some(result));
        }
        
        Ok(None)
    }
}

pub struct SecretManager {
    resolvers: Vec<Box<dyn SecretResolver>>,
}

pub struct ResolvedSecrets {
    pub env_vars: HashMap<String, String>,
    pub secret_values: HashSet<String>,
}

impl SecretManager {
    pub fn new() -> Self {
        Self {
            resolvers: vec![
                Box::new(OnePasswordResolver::new()),
                Box::new(GcpSecretResolver::new()),
            ],
        }
    }
    
    pub async fn resolve_secrets(&self, env_vars: HashMap<String, String>) -> Result<ResolvedSecrets> {
        let mut resolved_env = HashMap::new();
        let mut secret_values = HashSet::new();
        
        for (key, value) in env_vars {
            let mut resolved_value = value.clone();
            
            // Try each resolver
            for resolver in &self.resolvers {
                if let Some(secret) = resolver.resolve(&value).await? {
                    resolved_value = secret.clone();
                    secret_values.insert(secret);
                    log::debug!("Resolved secret for {}", key);
                    break;
                }
            }
            
            resolved_env.insert(key, resolved_value);
        }
        
        Ok(ResolvedSecrets {
            env_vars: resolved_env,
            secret_values,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    
    #[test]
    fn test_onepassword_regex() {
        let matches = ONEPASSWORD_REGEX.captures("op://Personal/GitHub/token");
        assert!(matches.is_some());
        let matches = matches.unwrap();
        assert_eq!(matches.get(1).unwrap().as_str(), "Personal");
        assert_eq!(matches.get(2).unwrap().as_str(), "GitHub");
        assert_eq!(matches.get(3).unwrap().as_str(), "token");
        
        let matches = ONEPASSWORD_REGEX.captures("op://Work/database-password");
        assert!(matches.is_some());
        let matches = matches.unwrap();
        assert_eq!(matches.get(1).unwrap().as_str(), "Work");
        assert_eq!(matches.get(2).unwrap().as_str(), "database-password");
        assert!(matches.get(3).is_none());
    }
    
    #[test]
    fn test_gcp_secret_regex() {
        let matches = GCP_SECRET_REGEX.captures("gcp-secret://my-project/api-key/1");
        assert!(matches.is_some());
        let matches = matches.unwrap();
        assert_eq!(matches.get(1).unwrap().as_str(), "my-project");
        assert_eq!(matches.get(2).unwrap().as_str(), "api-key");
        assert_eq!(matches.get(3).unwrap().as_str(), "1");
        
        let matches = GCP_SECRET_REGEX.captures("gcp-secret://prod-project/database-password");
        assert!(matches.is_some());
        let matches = matches.unwrap();
        assert_eq!(matches.get(1).unwrap().as_str(), "prod-project");
        assert_eq!(matches.get(2).unwrap().as_str(), "database-password");
        assert!(matches.get(3).is_none());
    }

    // Mock resolver for testing
    struct MockSecretResolver {
        secrets: HashMap<String, String>,
    }

    impl MockSecretResolver {
        fn new() -> Self {
            let mut secrets = HashMap::new();
            secrets.insert("op://Personal/database/password".to_string(), "secret-db-pass".to_string());
            secrets.insert("op://Work/api-key".to_string(), "secret-api-key".to_string());
            secrets.insert("gcp-secret://my-project/stripe-key".to_string(), "sk_test_123".to_string());
            secrets.insert("gcp-secret://prod/jwt-secret/1".to_string(), "jwt-secret-value".to_string());
            
            Self { secrets }
        }
    }

    #[async_trait]
    impl SecretResolver for MockSecretResolver {
        async fn resolve(&self, reference: &str) -> Result<Option<String>> {
            Ok(self.secrets.get(reference).cloned())
        }
    }

    #[tokio::test]
    async fn test_secret_manager_resolution() {
        let mut manager = SecretManager::new();
        // Replace resolvers with mock
        manager.resolvers = vec![Box::new(MockSecretResolver::new())];
        
        let mut env_vars = HashMap::new();
        env_vars.insert("NORMAL_VAR".to_string(), "plain-value".to_string());
        env_vars.insert("DB_PASS".to_string(), "op://Personal/database/password".to_string());
        env_vars.insert("API_KEY".to_string(), "op://Work/api-key".to_string());
        env_vars.insert("STRIPE_KEY".to_string(), "gcp-secret://my-project/stripe-key".to_string());
        env_vars.insert("JWT_SECRET".to_string(), "gcp-secret://prod/jwt-secret/1".to_string());
        env_vars.insert("UNRESOLVED".to_string(), "op://Unknown/item".to_string());
        
        let resolved = manager.resolve_secrets(env_vars).await.unwrap();
        
        // Normal variables should pass through unchanged
        assert_eq!(resolved.env_vars.get("NORMAL_VAR").unwrap(), "plain-value");
        
        // Secret references should be resolved
        assert_eq!(resolved.env_vars.get("DB_PASS").unwrap(), "secret-db-pass");
        assert_eq!(resolved.env_vars.get("API_KEY").unwrap(), "secret-api-key");
        assert_eq!(resolved.env_vars.get("STRIPE_KEY").unwrap(), "sk_test_123");
        assert_eq!(resolved.env_vars.get("JWT_SECRET").unwrap(), "jwt-secret-value");
        
        // Unresolved secrets should pass through as-is
        assert_eq!(resolved.env_vars.get("UNRESOLVED").unwrap(), "op://Unknown/item");
        
        // Check that secret values are tracked
        assert!(resolved.secret_values.contains("secret-db-pass"));
        assert!(resolved.secret_values.contains("secret-api-key"));
        assert!(resolved.secret_values.contains("sk_test_123"));
        assert!(resolved.secret_values.contains("jwt-secret-value"));
        assert!(!resolved.secret_values.contains("plain-value"));
    }

    #[tokio::test]
    async fn test_secret_manager_with_no_secrets() {
        // Create manager with no resolvers
        let manager = SecretManager { resolvers: vec![] };
        
        let mut env_vars = HashMap::new();
        env_vars.insert("VAR1".to_string(), "value1".to_string());
        env_vars.insert("VAR2".to_string(), "value2".to_string());
        
        let resolved = manager.resolve_secrets(env_vars.clone()).await.unwrap();
        
        // All variables should pass through unchanged
        assert_eq!(resolved.env_vars, env_vars);
        assert!(resolved.secret_values.is_empty());
    }
}