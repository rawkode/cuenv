use cuenv_core::{Result, TaskSecurity as TaskSecurityConfig};
use std::process::Command;

/// Apply security restrictions to a command
/// Returns Some(exit_code) if audit mode completed, None to continue execution
pub fn apply_security_restrictions(
    cmd: &mut Command,
    security: &TaskSecurityConfig,
    audit_mode: bool,
) -> Result<Option<i32>> {
    apply_security_restrictions_with_format(cmd, security, audit_mode, false)
}

/// Apply security restrictions to a command with output format control
/// Returns Some(exit_code) if audit mode completed, None to continue execution
pub fn apply_security_restrictions_with_format(
    cmd: &mut Command,
    security: &TaskSecurityConfig,
    audit_mode: bool,
    json_output: bool,
) -> Result<Option<i32>> {
    use cuenv_security::AccessRestrictions;
    let mut restrictions =
        AccessRestrictions::new(security.restrict_disk, security.restrict_network);

    // Add allowed paths
    for path in &security.read_only_paths {
        restrictions.add_read_only_path(path);
    }
    for path in &security.write_only_paths {
        restrictions.add_read_write_path(path);
    }

    // Add allowed hosts for network filtering
    for host in &security.allowed_hosts {
        restrictions.add_allowed_host(host);
    }

    if audit_mode {
        restrictions.enable_audit_mode();
        // TODO: Add tracing when moved to workspace
        // task_progress(task_name, None, "Running task in audit mode...");

        let (exit_code, audit_report) = restrictions.run_with_audit(cmd)?;

        if json_output {
            match audit_report.to_json() {
                Ok(json) => println!("{json}"),
                Err(e) => {
                    eprintln!("Warning: Failed to export audit report as JSON: {e}");
                    audit_report.print_summary();
                }
            }
        } else {
            audit_report.print_summary();
        }

        return Ok(Some(exit_code));
    } else if restrictions.has_any_restrictions() {
        restrictions.apply_to_command(cmd)?;
    }

    Ok(None)
}
