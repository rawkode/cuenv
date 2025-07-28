#![allow(unused)]
use cuenv::access_restrictions::AccessRestrictions;
use proptest::prelude::*;
use std::path::PathBuf;

// Generate valid paths for testing
fn arb_path() -> impl Strategy<Value = PathBuf> {
    prop::collection::vec(
        prop_oneof![
            "[a-zA-Z0-9]+".prop_map(|s| s.to_string()),
            Just(".".to_string()),
            Just("..".to_string()),
        ],
        1..5,
    )
    .prop_map(|parts| {
        let mut path = PathBuf::new();
        for part in parts {
            path.push(part);
        }
        path
    })
}

// Generate lists of paths
fn arb_path_list() -> impl Strategy<Value = Vec<PathBuf>> {
    prop::collection::vec(arb_path(), 0..10)
}

// Generate host patterns
fn arb_host() -> impl Strategy<Value = String> {
    prop_oneof![
        // Domain names
        "[a-z][a-z0-9-]*\\.[a-z]{2,}",
        // IP addresses (simplified)
        "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}",
        // CIDR blocks
        "[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}\\.[0-9]{1,3}/[0-9]{1,2}",
        // Wildcards
        "\\*\\.[a-z][a-z0-9-]*\\.[a-z]{2,}",
    ]
}

// Generate lists of hosts
fn arb_host_list() -> impl Strategy<Value = Vec<String>> {
    prop::collection::vec(arb_host(), 0..10)
}

proptest! {
    #[test]
    fn test_access_restrictions_creation(
        restrict_disk in any::<bool>(),
        restrict_network in any::<bool>(),
        read_only_paths in arb_path_list(),
        read_write_paths in arb_path_list(),
        deny_paths in arb_path_list(),
        allowed_hosts in arb_host_list(),
    ) {
        let restrictions = AccessRestrictions::with_allowlists(
            restrict_disk,
            restrict_network,
            read_only_paths.clone(),
            read_write_paths.clone(),
            deny_paths.clone(),
            allowed_hosts.clone(),
        );

        // Verify all fields are set correctly
        prop_assert_eq!(restrictions.restrict_disk, restrict_disk);
        prop_assert_eq!(restrictions.restrict_network, restrict_network);
        prop_assert_eq!(restrictions.read_only_paths, read_only_paths);
        prop_assert_eq!(restrictions.read_write_paths, read_write_paths);
        prop_assert_eq!(restrictions.deny_paths, deny_paths);
        prop_assert_eq!(restrictions.allowed_hosts, allowed_hosts);
        prop_assert!(!restrictions.audit_mode);
    }

    #[test]
    fn test_path_manipulation(
        initial_paths in arb_path_list(),
        additional_paths in arb_path_list(),
    ) {
        let mut restrictions = AccessRestrictions::new(true, false);

        // Add initial paths as read-only
        for path in &initial_paths {
            restrictions.add_read_only_path(path.clone());
        }

        // Add additional paths as read-write
        for path in &additional_paths {
            restrictions.add_read_write_path(path.clone());
        }

        // Verify all paths were added
        prop_assert_eq!(restrictions.read_only_paths.len(), initial_paths.len());
        prop_assert_eq!(restrictions.read_write_paths.len(), additional_paths.len());

        // Verify path contents match
        for (i, path) in initial_paths.iter().enumerate() {
            prop_assert_eq!(&restrictions.read_only_paths[i], path);
        }
        for (i, path) in additional_paths.iter().enumerate() {
            prop_assert_eq!(&restrictions.read_write_paths[i], path);
        }
    }

    #[test]
    fn test_has_any_restrictions(
        restrict_disk in any::<bool>(),
        restrict_network in any::<bool>(),
    ) {
        let restrictions = AccessRestrictions::new(restrict_disk, restrict_network);
        prop_assert_eq!(
            restrictions.has_any_restrictions(),
            restrict_disk || restrict_network
        );
    }

    #[test]
    fn test_audit_mode_toggle(
        restrict_disk in any::<bool>(),
        restrict_network in any::<bool>(),
    ) {
        let mut restrictions = AccessRestrictions::new(restrict_disk, restrict_network);

        // Initially audit mode should be off
        prop_assert!(!restrictions.audit_mode);

        // Enable audit mode
        restrictions.enable_audit_mode();
        prop_assert!(restrictions.audit_mode);
    }

    #[test]
    fn test_allowed_hosts_manipulation(
        initial_hosts in arb_host_list(),
        additional_hosts in arb_host_list(),
    ) {
        let mut restrictions = AccessRestrictions::new(false, true);

        // Add initial hosts
        for host in &initial_hosts {
            restrictions.add_allowed_host(host.clone());
        }

        // Verify initial hosts were added
        prop_assert_eq!(restrictions.allowed_hosts.len(), initial_hosts.len());

        // Add additional hosts
        for host in &additional_hosts {
            restrictions.add_allowed_host(host.clone());
        }

        // Verify all hosts were added
        prop_assert_eq!(
            restrictions.allowed_hosts.len(),
            initial_hosts.len() + additional_hosts.len()
        );
    }
}

#[test]
fn test_default_access_restrictions() {
    let restrictions = AccessRestrictions::default();

    // Default should have no restrictions
    assert!(!restrictions.restrict_disk);
    assert!(!restrictions.restrict_network);
    assert!(restrictions.read_only_paths.is_empty());
    assert!(restrictions.read_write_paths.is_empty());
    assert!(restrictions.deny_paths.is_empty());
    assert!(restrictions.allowed_hosts.is_empty());
    assert!(!restrictions.audit_mode);
    assert!(!restrictions.has_any_restrictions());
}

proptest! {
    #[test]
    fn test_from_security_config_inference(
        inputs in prop::option::of(prop::collection::vec("[a-zA-Z0-9/._-]+", 0..5)),
        outputs in prop::option::of(prop::collection::vec("[a-zA-Z0-9/._-]+", 0..5)),
        infer in any::<bool>(),
        restrict_disk in prop::option::of(any::<bool>()),
        restrict_network in prop::option::of(any::<bool>()),
    ) {
        use cuenv::cue_parser::{SecurityConfig, TaskConfig};

        let security = SecurityConfig {
            restrict_disk,
            restrict_network,
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: Some(infer),
        };

        let task_config = TaskConfig {
            command: Some("test".to_string()),
            description: None,
            script: None,
            dependencies: None,
            working_dir: None,
            shell: None,
            inputs: inputs.clone(),
            outputs: outputs.clone(),
            security: None,
            cache: None,
            cache_key: None,
            timeout: None,
        };

        let restrictions = AccessRestrictions::from_security_config_with_task(&security, &task_config);

        if infer {
            // If inference is enabled and we have inputs/outputs, disk restrictions should be enabled
            if inputs.is_some() || outputs.is_some() {
                prop_assert!(restrictions.restrict_disk);
            }

            // Check that inputs were added as read-only paths
            if let Some(ref input_paths) = inputs {
                prop_assert_eq!(restrictions.read_only_paths.len(), input_paths.len());
            }

            // Check that outputs were added as read-write paths
            if let Some(ref output_paths) = outputs {
                prop_assert_eq!(restrictions.read_write_paths.len(), output_paths.len());
            }
        } else {
            // Without inference, paths should be empty
            prop_assert!(restrictions.read_only_paths.is_empty());
            prop_assert!(restrictions.read_write_paths.is_empty());
        }
    }
}
