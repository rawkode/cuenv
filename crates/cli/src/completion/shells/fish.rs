//! Fish shell completion generator

use cuenv_core::Result;

/// Generate fish completion script
pub fn generate() -> Result<()> {
    let script = r#"
function __fish_cuenv_using_command
    set cmd (commandline -opc)
    test (count $cmd) -ge 2; and test $cmd[2] = $argv[1]
end

function __fish_cuenv_using_task_subcommand
    set cmd (commandline -opc)
    test (count $cmd) -ge 3; and begin
        test $cmd[2] = "task"; or test $cmd[2] = "t"
    end; and test $cmd[3] = $argv[1]
end

# Main commands
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "task t" -d "Manage and execute tasks"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "env e" -d "Manage environment configuration"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "init" -d "Initialize a new env.cue file"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "discover" -d "Discover all CUE packages"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "cache" -d "Cache management"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "shell" -d "Shell integration"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "completion" -d "Generate completion scripts"

# Task subcommands
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "list l" -d "List available tasks"
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "run r" -d "Run a task"
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "exec e" -d "Execute a command"

# Task names for task run
complete -f -c cuenv -n "__fish_cuenv_using_task_subcommand run; or __fish_cuenv_using_task_subcommand r" -a "(cuenv _complete_tasks 2>/dev/null)"

# Global options
complete -f -c cuenv -s h -l help -d "Print help information"
complete -f -c cuenv -s V -l version -d "Print version information"
complete -f -c cuenv -s e -l env -d "Environment to use"
complete -f -c cuenv -s c -l capability -d "Capabilities to enable"
complete -f -c cuenv -l audit -d "Run in audit mode"
"#;
    tracing::info!("{script}");
    Ok(())
}
