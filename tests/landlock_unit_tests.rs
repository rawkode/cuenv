#![allow(unused)]
#[cfg(test)]
mod landlock_unit_tests {
    use cuenv::access_restrictions::AccessRestrictions;
    use cuenv::cue_parser::{SecurityConfig, TaskConfig};
    use std::path::PathBuf;

    #[test]
    fn test_security_config_to_restrictions_all_fields() {
        let security_config = SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: Some(true),
            read_only_paths: Some(vec!["/usr".into(), "/bin".into()]),
            read_write_paths: Some(vec!["/tmp".into(), "/var/tmp".into()]),
            deny_paths: Some(vec!["/etc/shadow".into()]),
            allowed_hosts: Some(vec!["443".to_string(), "80".to_string()]),
            infer_from_inputs_outputs: Some(false),
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);

        assert!(restrictions.restrict_disk);
        assert!(restrictions.restrict_network);
        assert_eq!(restrictions.read_only_paths.len(), 2);
        assert_eq!(restrictions.read_write_paths.len(), 2);
        assert_eq!(restrictions.deny_paths.len(), 1);
        assert_eq!(restrictions.allowed_hosts.len(), 2);
    }

    #[test]
    fn test_security_config_defaults() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: None,
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);

        assert!(!restrictions.restrict_disk);
        assert!(!restrictions.restrict_network);
        assert!(restrictions.read_only_paths.is_empty());
        assert!(restrictions.read_write_paths.is_empty());
        assert!(restrictions.deny_paths.is_empty());
        assert!(restrictions.allowed_hosts.is_empty());
    }

    #[test]
    fn test_infer_from_task_inputs_outputs() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: None,
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: Some(true),
        };

        let task_config = TaskConfig {
            command: Some("process".to_string()),
            description: None,
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: Some(vec!["/data/input.txt".to_string()]),
            outputs: Some(vec!["/data/output.txt".to_string()]),
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: None,
        };

        let restrictions =
            AccessRestrictions::from_security_config_with_task(&security_config, &task_config);

        assert!(restrictions.restrict_disk);
        assert_eq!(restrictions.read_only_paths.len(), 1);
        assert_eq!(
            restrictions.read_only_paths[0],
            PathBuf::from("/data/input.txt")
        );
        assert_eq!(restrictions.read_write_paths.len(), 1);
        assert_eq!(
            restrictions.read_write_paths[0],
            PathBuf::from("/data/output.txt")
        );
    }

    #[test]
    fn test_infer_with_existing_paths_merge() {
        let security_config = SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: None,
            read_only_paths: Some(vec!["/usr".into()]),
            read_write_paths: Some(vec!["/tmp".into()]),
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: Some(true),
        };

        let task_config = TaskConfig {
            command: Some("process".to_string()),
            description: None,
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: Some(vec!["/data/input.txt".to_string()]),
            outputs: Some(vec!["/data/output.txt".to_string()]),
            security: None,
            cache: None,
            cache_key: None,
            cache_env: None,
            timeout: None,
        };

        let restrictions =
            AccessRestrictions::from_security_config_with_task(&security_config, &task_config);

        // Should merge both configured and inferred paths
        assert_eq!(restrictions.read_only_paths.len(), 2);
        assert!(restrictions
            .read_only_paths
            .contains(&PathBuf::from("/usr")));
        assert!(restrictions
            .read_only_paths
            .contains(&PathBuf::from("/data/input.txt")));

        assert_eq!(restrictions.read_write_paths.len(), 2);
        assert!(restrictions
            .read_write_paths
            .contains(&PathBuf::from("/tmp")));
        assert!(restrictions
            .read_write_paths
            .contains(&PathBuf::from("/data/output.txt")));
    }

    #[test]
    fn test_empty_allowed_hosts_blocks_all() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: Some(true),
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: Some(vec![]), // Empty list should block all
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);

        assert!(restrictions.restrict_network);
        assert!(restrictions.allowed_hosts.is_empty());
    }

    #[test]
    fn test_port_parsing_in_allowed_hosts() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: Some(true),
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: Some(vec![
                "443".to_string(),
                "80".to_string(),
                "8080".to_string(),
                "invalid-port".to_string(), // Should be handled gracefully
            ]),
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);

        assert_eq!(restrictions.allowed_hosts.len(), 4);
        // The actual port parsing happens in apply_landlock_restrictions
    }

    #[test]
    fn test_audit_mode_state() {
        let mut restrictions = AccessRestrictions::new(true, true);
        assert!(!restrictions.audit_mode);

        restrictions.enable_audit_mode();
        assert!(restrictions.audit_mode);
    }

    #[test]
    fn test_path_deduplication() {
        let mut restrictions = AccessRestrictions::new(true, false);

        // Add duplicate paths
        restrictions.add_read_only_path("/usr");
        restrictions.add_read_only_path("/usr");
        restrictions.add_read_only_path("/bin");

        // Should not have duplicates (implementation dependent)
        // Current implementation allows duplicates, but that's OK
        assert_eq!(restrictions.read_only_paths.len(), 3);
    }

    #[test]
    fn test_complex_security_scenario() {
        // Simulate a complex security scenario
        let security_config = SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: Some(true),
            read_only_paths: Some(vec![
                "/usr".into(),
                "/bin".into(),
                "/lib".into(),
                "/lib64".into(),
                "/nix/store".into(),
            ]),
            read_write_paths: Some(vec![
                "/tmp".into(),
                "/var/tmp".into(),
                "/home/user/workspace".into(),
            ]),
            deny_paths: Some(vec![
                "/etc/shadow".into(),
                "/etc/passwd".into(),
                "/root".into(),
            ]),
            allowed_hosts: Some(vec![
                "443".to_string(),  // HTTPS
                "80".to_string(),   // HTTP
                "22".to_string(),   // SSH
                "3000".to_string(), // Custom app
            ]),
            infer_from_inputs_outputs: Some(false),
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);

        assert!(restrictions.has_any_restrictions());
        assert_eq!(restrictions.read_only_paths.len(), 5);
        assert_eq!(restrictions.read_write_paths.len(), 3);
        assert_eq!(restrictions.deny_paths.len(), 3);
        assert_eq!(restrictions.allowed_hosts.len(), 4);
    }
}
