#![allow(unused)]
#[cfg(all(test, target_os = "linux"))]
mod landlock_tests {
    use cuenv::access_restrictions::AccessRestrictions;
    use cuenv::cue_parser::SecurityConfig;
    use std::fs;
    use std::path::PathBuf;
    use std::process::Command;

    #[test]
    #[ignore] // Requires Landlock support in kernel
    fn test_filesystem_restriction_blocks_access() {
        let security_config = SecurityConfig {
            restrict_disk: Some(true),
            restrict_network: None,
            read_only_paths: Some(vec!["/usr".into(), "/bin".into()]),
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None,
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);
        let mut cmd = Command::new("cat");
        cmd.arg("/etc/passwd");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should fail with permission denied
        assert!(!output.status.success());
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("Permission denied"));
    }

    #[test]
    #[ignore] // Requires Landlock support in kernel
    fn test_network_restriction_blocks_connections() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: Some(true),
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: None, // No allowed hosts = block all
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);
        let mut cmd = Command::new("curl");
        cmd.args(&["-I", "--max-time", "2", "https://www.google.com"]);

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should fail to connect
        assert!(!output.status.success());
    }

    #[test]
    #[ignore] // Requires Landlock support in kernel
    fn test_allowed_port_permits_connection() {
        let security_config = SecurityConfig {
            restrict_disk: None,
            restrict_network: Some(true),
            read_only_paths: None,
            read_write_paths: None,
            deny_paths: None,
            allowed_hosts: Some(vec!["443".to_string()]),
            infer_from_inputs_outputs: None,
        };

        let restrictions = AccessRestrictions::from_security_config(&security_config);
        let mut cmd = Command::new("curl");
        cmd.args(&["-I", "--max-time", "5", "https://www.google.com"]);

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should succeed with HTTPS on port 443
        assert!(output.status.success());
    }
}
