/// Constants used throughout the cuenv codebase
// CUE package constants
pub const ENV_CUE_FILENAME: &str = "env.cue";
pub const CUENV_PACKAGE_VAR: &str = "CUENV_PACKAGE";
pub const DEFAULT_PACKAGE_NAME: &str = "cuenv";

// Resolver prefix
pub const CUENV_RESOLVER_PREFIX: &str = "cuenv-resolver://";

// Environment variable names
pub const CUENV_ENV_VAR: &str = "CUENV_ENV";
pub const CUENV_CAPABILITIES_VAR: &str = "CUENV_CAPABILITIES";
pub const CUENV_LOG_VAR: &str = "CUENV_LOG";

// Default shell
pub const DEFAULT_SHELL: &str = "bash";

// System paths to filter in audit mode
pub const AUDIT_IGNORED_PATH_PREFIXES: &[&str] = &["/proc/", "/sys/", "/dev/", "/tmp/"];

// Common system files
pub const LD_SO_CACHE: &str = "/etc/ld.so.cache";

// Audit log path
pub const AUDIT_LOG_PATH: &str = "/tmp/cuenv_audit.log";

// Network access patterns
pub const LOCALHOST_PATTERN: &str = "localhost";
