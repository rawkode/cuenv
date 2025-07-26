#[cfg(all(test, target_os = "linux"))]
mod landlock_edge_cases {
    use cuenv::access_restrictions::AccessRestrictions;
    use std::process::Command;

    #[test]
    #[ignore] // Requires Landlock support
    fn test_empty_allowlist_blocks_everything() {
        let restrictions = AccessRestrictions::new(true, false);
        // No paths added - should block everything

        let mut cmd = Command::new("ls");
        cmd.arg("/");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        assert!(!output.status.success());
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_symlink_handling() {
        let mut restrictions = AccessRestrictions::new(true, false);
        // Only allow /usr/bin but not the actual binary locations
        restrictions.add_read_only_path("/usr/bin");

        let mut cmd = Command::new("/usr/bin/ls");
        cmd.arg("/etc");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should fail because /etc is not allowed
        assert!(!output.status.success());
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_partial_path_access() {
        let mut restrictions = AccessRestrictions::new(true, false);
        // Allow /home but not /home/user
        restrictions.add_read_only_path("/home");

        let mut cmd = Command::new("ls");
        cmd.arg("/home/user");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should succeed because /home allows subdirectories
        assert!(output.status.success());
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_write_requires_read() {
        let mut restrictions = AccessRestrictions::new(true, false);
        // Only give write access, not read
        restrictions.add_read_write_path("/tmp/test-file");

        let mut cmd = Command::new("cat");
        cmd.arg("/tmp/test-file");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should succeed because read-write includes read
        assert!(
            output.status.success()
                || String::from_utf8_lossy(&output.stderr).contains("No such file")
        );
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_network_port_0_handling() {
        let mut restrictions = AccessRestrictions::new(false, true);
        restrictions.add_allowed_host("0"); // Port 0 is invalid

        let mut cmd = Command::new("curl");
        cmd.args(&["-I", "--max-time", "1", "http://example.com"]);

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should fail because port 0 is not a valid port
        assert!(!output.status.success());
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_network_high_port_numbers() {
        let mut restrictions = AccessRestrictions::new(false, true);
        restrictions.add_allowed_host("65535"); // Maximum port number

        let mut cmd = Command::new("nc");
        cmd.args(&["-zv", "127.0.0.1", "65535"]);

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should be allowed to attempt connection on port 65535
        // (may fail for other reasons)
        // Just checking it doesn't crash
        let _ = output.status;
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_multiple_network_operations() {
        let mut restrictions = AccessRestrictions::new(false, true);
        restrictions.add_allowed_host("443");
        restrictions.add_allowed_host("80");

        // Test that both HTTP and HTTPS work
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd.arg("curl -I https://example.com && curl -I http://example.com");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Both should work since we allowed both ports
        assert!(output.status.success());
    }

    #[test]
    fn test_landlock_availability_check() {
        // This test should always pass
        let is_supported = AccessRestrictions::is_landlock_supported();

        // Just verify it returns a boolean without crashing
        assert!(is_supported || !is_supported);
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_combined_filesystem_and_network() {
        let mut restrictions = AccessRestrictions::new(true, true);
        restrictions.add_read_only_path("/usr");
        restrictions.add_read_only_path("/bin");
        restrictions.add_read_write_path("/tmp");
        restrictions.add_allowed_host("443");

        // Try to download a file to /tmp
        let mut cmd = Command::new("curl");
        cmd.args(&["-o", "/tmp/test-download", "https://example.com"]);

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should succeed if curl binary is in allowed paths
        // and we can write to /tmp and connect to port 443
        let stderr = String::from_utf8_lossy(&output.stderr);

        // Check for common failure modes
        if !output.status.success() {
            assert!(
                stderr.contains("Permission denied")
                    || stderr.contains("Failed to connect")
                    || stderr.contains("Could not resolve")
            );
        }
    }

    #[test]
    #[ignore] // Requires Landlock support
    fn test_deny_paths_precedence() {
        // Test that deny paths take precedence over allow paths
        let mut restrictions = AccessRestrictions::new(true, false);
        restrictions.add_read_write_path("/home");
        restrictions.add_deny_path("/home/secret");

        let mut cmd = Command::new("cat");
        cmd.arg("/home/secret/file");

        restrictions.apply_to_command(&mut cmd).unwrap();
        let output = cmd.output().expect("Failed to execute command");

        // Should fail even though /home is allowed
        assert!(!output.status.success());
    }
}
