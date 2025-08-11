//! Unit tests for the Config module

#[cfg(test)]
mod tests {
    use crate::{
        config::{Config, ConfigBuilder, MonorepoContext, RuntimeOptions},
        ParseResult, SecurityConfig, VariableMetadata,
    };
    use std::collections::HashMap;
    use std::path::PathBuf;

    fn create_test_parse_result() -> ParseResult {
        let mut variables = HashMap::new();
        variables.insert("TEST_VAR".to_string(), "test_value".to_string());
        variables.insert("ANOTHER_VAR".to_string(), "another_value".to_string());

        let mut metadata = HashMap::new();
        metadata.insert(
            "TEST_VAR".to_string(),
            VariableMetadata {
                capability: Some("basic".to_string()),
            },
        );
        metadata.insert(
            "SECRET_VAR".to_string(),
            VariableMetadata {
                capability: Some("secrets".to_string()),
            },
        );

        ParseResult {
            variables,
            metadata,
            commands: HashMap::new(),
            tasks: HashMap::new(),
            hooks: HashMap::new(),
        }
    }

    #[test]
    fn test_config_new() {
        let working_dir = PathBuf::from("/test/dir");
        let env_file = Some(PathBuf::from("/test/dir/env.cue"));
        let parse_result = create_test_parse_result();
        let runtime = RuntimeOptions::default();

        let config = Config::new(working_dir.clone(), env_file.clone(), parse_result, runtime);

        assert_eq!(config.working_dir, working_dir);
        assert_eq!(config.env_file, env_file);
        assert_eq!(config.parse_result.variables.len(), 2);
    }

    #[test]
    fn test_config_get_env_vars() {
        let config = Config::new(
            PathBuf::from("/test"),
            None,
            create_test_parse_result(),
            RuntimeOptions::default(),
        );

        let vars = config.get_env_vars().expect("Failed to get env vars");
        assert_eq!(vars.get("TEST_VAR"), Some(&"test_value".to_string()));
        assert_eq!(vars.get("ANOTHER_VAR"), Some(&"another_value".to_string()));
    }

    #[test]
    fn test_config_is_sensitive() {
        let config = Config::new(
            PathBuf::from("/test"),
            None,
            create_test_parse_result(),
            RuntimeOptions::default(),
        );

        // TODO: When sensitive field is added to VariableMetadata, update this test
        assert!(!config.is_sensitive("TEST_VAR"));
        assert!(!config.is_sensitive("SECRET_VAR"));
        assert!(!config.is_sensitive("NONEXISTENT"));
    }

    #[test]
    fn test_config_builder() {
        let working_dir = PathBuf::from("/test/dir");
        let parse_result = create_test_parse_result();

        let config = ConfigBuilder::new()
            .working_dir(working_dir.clone())
            .parse_result(parse_result)
            .environment("production".to_string())
            .capabilities(vec!["docker".to_string(), "network".to_string()])
            .audit_mode(true)
            .build()
            .expect("Failed to build config");

        assert_eq!(config.working_dir, working_dir);
        assert_eq!(config.runtime.environment, Some("production".to_string()));
        assert_eq!(config.runtime.capabilities.len(), 2);
        assert!(config.runtime.audit_mode);
    }

    #[test]
    fn test_config_builder_missing_required() {
        let result = ConfigBuilder::new().build();
        assert!(result.is_err());
    }

    #[test]
    fn test_config_with_monorepo() {
        let mut config = Config::new(
            PathBuf::from("/test/packages/pkg1"),
            None,
            create_test_parse_result(),
            RuntimeOptions::default(),
        );

        let monorepo = MonorepoContext {
            root_dir: PathBuf::from("/test"),
            current_package: Some("pkg1".to_string()),
            packages: HashMap::from([
                ("pkg1".to_string(), PathBuf::from("/test/packages/pkg1")),
                ("pkg2".to_string(), PathBuf::from("/test/packages/pkg2")),
            ]),
        };

        config.monorepo = Some(monorepo);

        assert!(config.is_monorepo());
        assert_eq!(config.monorepo_root(), Some(&PathBuf::from("/test")));
    }

    #[test]
    fn test_config_with_security() {
        let mut config = Config::new(
            PathBuf::from("/test"),
            None,
            create_test_parse_result(),
            RuntimeOptions::default(),
        );

        config.security = SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: Some(true),
            read_only_paths: Some(vec!["/etc".to_string()]),
            read_write_paths: Some(vec!["/tmp".to_string()]),
            deny_paths: Some(vec!["/secret".to_string()]),
            allowed_hosts: Some(vec!["github.com".to_string()]),
            infer_from_inputs_outputs: Some(false),
        };

        assert_eq!(config.security.restrict_disk, Some(true));
        assert_eq!(config.security.restrict_network, Some(true));
        assert_eq!(
            config.security.read_only_paths.as_ref().map(|v| v.len()),
            Some(1)
        );
        assert_eq!(
            config.security.allowed_hosts.as_ref().map(|v| v.len()),
            Some(1)
        );
    }

    #[test]
    fn test_runtime_options_default() {
        let runtime = RuntimeOptions::default();

        assert_eq!(runtime.environment, None);
        assert!(runtime.capabilities.is_empty());
        assert_eq!(runtime.cache_mode, None);
        assert!(runtime.cache_enabled);
        assert!(!runtime.audit_mode);
    }

    #[test]
    fn test_config_arc_conversion() {
        let config = Config::new(
            PathBuf::from("/test"),
            None,
            create_test_parse_result(),
            RuntimeOptions::default(),
        );

        let arc_config = config.into_arc();
        assert_eq!(arc_config.working_dir, PathBuf::from("/test"));

        // Can clone Arc without duplicating data
        let arc_clone = arc_config.clone();
        assert_eq!(arc_clone.working_dir, PathBuf::from("/test"));
    }

    #[test]
    fn test_config_get_hooks() {
        let mut parse_result = create_test_parse_result();

        // Add some test hooks
        use crate::Hook;

        let hook = Hook {
            command: "echo".to_string(),
            args: Some(vec!["test".to_string()]),
            dir: None,
            inputs: None,
            source: None,
        };

        parse_result.hooks.insert("onEnter".to_string(), vec![hook]);

        let config = Config::new(
            PathBuf::from("/test"),
            None,
            parse_result,
            RuntimeOptions::default(),
        );

        let hooks = config.get_hooks("onEnter");
        assert_eq!(hooks.len(), 1);

        let no_hooks = config.get_hooks("onExit");
        assert_eq!(no_hooks.len(), 0);
    }
}
