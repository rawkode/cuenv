#![allow(unused)]
#[cfg(all(test, target_os = "linux"))]
mod landlock_audit_tests {
    use cuenv::access_restrictions::AccessRestrictions;
    use std::fs;
    use std::path::Path;
    use std::process::Command;

    #[test]
    #[ignore] // Requires strace to be installed
    fn test_audit_mode_basic() {
        let mut restrictions = AccessRestrictions::new(true, true);
        restrictions.enable_audit_mode();

        let mut cmd = Command::new("cat");
        cmd.arg("/etc/hostname");

        match restrictions.run_with_audit(&mut cmd) {
            Ok((exit_code, report)) => {
                // The command should have succeeded
                assert_eq!(exit_code, 0);

                // Should have detected file access
                assert!(!report.accessed_files.is_empty());

                // Should include /etc/hostname in accessed files
                let accessed_paths: Vec<&str> =
                    report.accessed_files.iter().map(|s| s.as_str()).collect();

                // May include the binary and libraries too
                assert!(accessed_paths
                    .iter()
                    .any(|&p| p.contains("/etc/hostname") || p.contains("hostname")));
            }
            Err(e) => {
                // If strace is not available, that's OK for CI
                eprintln!("Audit test skipped: {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Requires strace and network utilities
    fn test_audit_mode_network() {
        let mut restrictions = AccessRestrictions::new(false, true);
        restrictions.enable_audit_mode();

        let mut cmd = Command::new("ping");
        cmd.args(&["-c", "1", "-W", "1", "127.0.0.1"]);

        match restrictions.run_with_audit(&mut cmd) {
            Ok((_, report)) => {
                // Should have detected network access
                assert!(!report.network_connections.is_empty());

                // Should include localhost connection
                let has_localhost = report
                    .network_connections
                    .iter()
                    .any(|conn| conn.contains("127.0.0.1"));
                assert!(has_localhost);
            }
            Err(e) => {
                eprintln!("Audit test skipped: {}", e);
            }
        }
    }

    #[test]
    #[ignore] // Requires strace
    fn test_audit_mode_complex_command() {
        let mut restrictions = AccessRestrictions::new(true, true);
        restrictions.enable_audit_mode();

        // Run a command that accesses multiple files
        let mut cmd = Command::new("sh");
        cmd.arg("-c");
        cmd.arg("cat /etc/os-release && ls /tmp");

        match restrictions.run_with_audit(&mut cmd) {
            Ok((exit_code, report)) => {
                assert_eq!(exit_code, 0);

                // Should have multiple file accesses
                assert!(report.accessed_files.len() > 1);

                // Print summary for debugging
                report.print_summary();
            }
            Err(e) => {
                eprintln!("Audit test skipped: {}", e);
            }
        }
    }

    #[test]
    fn test_audit_report_summary() {
        use cuenv::access_restrictions::AuditReport;

        let report = AuditReport {
            accessed_files: vec![
                "/etc/passwd".to_string(),
                "/usr/lib/libc.so".to_string(),
                "/home/user/data.txt".to_string(),
            ],
            network_connections: vec![
                "tcp:example.com:443".to_string(),
                "udp:8.8.8.8:53".to_string(),
            ],
        };

        // Just test that print_summary doesn't panic
        report.print_summary();
    }

    #[test]
    fn test_empty_audit_report() {
        use cuenv::access_restrictions::AuditReport;

        let report = AuditReport {
            accessed_files: vec![],
            network_connections: vec![],
        };

        // Should handle empty report gracefully
        report.print_summary();
    }
}
