//! Smart defaults for common build tools and environments

/// Smart defaults for environment variable filtering
pub struct SmartDefaults;

impl SmartDefaults {
    /// Get smart defaults for common build tools
    pub fn get_defaults() -> (Vec<&'static str>, Vec<&'static str>) {
        // Default allowlist - include these by default
        let allowlist = vec![
            // Core system variables
            "PATH",
            "HOME",
            "USER",
            "SHELL",
            "LANG",
            "LC_*",
            // Build tool variables
            "CC",
            "CXX",
            "CPPFLAGS",
            "CFLAGS",
            "CXXFLAGS",
            "LDFLAGS",
            "MAKEFLAGS",
            "MAKELEVEL",
            "MFLAGS",
            // Rust/Cargo
            "CARGO_*",
            "RUST*",
            // Node.js/npm
            "npm_config_*",
            "NODE_*",
            "NPM_*",
            // Python
            "PYTHON*",
            "PIP_*",
            "VIRTUAL_ENV",
            // Go
            "GO*",
            "GOPATH",
            "GOROOT",
            // Java/Maven/Gradle
            "JAVA_*",
            "MAVEN_*",
            "GRADLE_*",
            // Docker
            "DOCKER_*",
            // Build systems
            "BUILD_*",
            "BAZEL_*",
            "NIX_*",
            // Version control
            "GIT_*",
            "SVN_*",
            "HG_*",
            // Package managers
            "APT_*",
            "YUM_*",
            "BREW_*",
            // Cross-platform build variables
            "OS",
            "ARCH",
            "TARGET",
            "HOST",
            // CI/CD variables
            "CI",
            "CONTINUOUS_INTEGRATION",
            "BUILD_NUMBER",
            "GITHUB_*",
            "GITLAB_*",
            "JENKINS_*",
            "TRAVIS_*",
            // Development tools
            "EDITOR",
            "VISUAL",
            "PAGER",
        ];

        // Default denylist - exclude these always for cross-platform consistency
        let denylist = vec![
            // Shell/session variables
            "PS1",
            "PS2",
            "PS3",
            "PS4",
            "TERM",
            "TERMCAP",
            "COLORTERM",
            "PWD",
            "OLDPWD",
            "SHLVL",
            "_",
            "SHELL_SESSION_ID",
            // Terminal/display (platform-specific)
            "DISPLAY",
            "WAYLAND_DISPLAY",
            "XDG_*",
            "DBUS_*",
            "SESSION_MANAGER",
            "XAUTHORITY",
            "WINDOWID",
            // History/temp files
            "HIST*",
            "LESS*",
            "MORE",
            "PAGER",
            "MANPAGER",
            "TMPDIR",
            "TEMP",
            "TMP",
            // User interface
            "LS_COLORS",
            "LSCOLORS",
            "CLICOLOR",
            "CLICOLOR_FORCE",
            // SSH/session specific
            "SSH_*",
            "SSH_CLIENT",
            "SSH_CONNECTION",
            "SSH_TTY",
            "WINDOW",
            "STY",
            "TMUX*",
            "SCREEN*",
            // Random/temporary
            "RANDOM",
            "LINENO",
            "SECONDS",
            "BASHPID",
            // Process specific
            "PPID",
            "UID",
            "EUID",
            "GID",
            "EGID",
            // Platform-specific session variables
            "HOSTNAME",
            "LOGNAME",
            "USERDOMAIN",
            "COMPUTERNAME",
            "USERNAME", // Windows-specific
            // Development environment specific (terminal-dependent)
            "VTE_VERSION",
            "WT_SESSION",
            "TERM_PROGRAM",
            "TERM_PROGRAM_VERSION",
            "ITERM_SESSION_ID",
            // macOS specific
            "__CF_USER_TEXT_ENCODING",
            "COMMAND_MODE",
            "SECURITYSESSIONID",
            // Linux specific
            "XDG_RUNTIME_DIR",
            "XDG_DATA_DIRS",
            "XDG_CONFIG_DIRS",
            // Windows specific (WSL/Cygwin)
            "WSL*",
            "WSL_DISTRO_NAME",
            "WSL_INTEROP",
            "CYGWIN*",
            "MSYS*",
        ];

        (allowlist, denylist)
    }
}
