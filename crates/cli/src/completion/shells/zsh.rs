//! Zsh shell completion generator

use cuenv_core::Result;

/// Generate zsh completion script
pub fn generate() -> Result<()> {
    let script = r#"
#compdef cuenv

_cuenv() {
    local context state line
    typeset -A opt_args
    
    _arguments -C \
        '1: :_cuenv_commands' \
        '*::arg:->args' \
        '(-h --help)'{-h,--help}'[Print help]' \
        '(-V --version)'{-V,--version}'[Print version]' \
        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
        '--audit[Run in audit mode]'
    
    case $state in
        args)
            case $words[1] in
                task|t)
                    _arguments \
                        '1: :_cuenv_task_commands' \
                        '*::arg:->task_args'
                    
                    case $state in
                        task_args)
                            case $words[1] in
                                run|r)
                                    _arguments \
                                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                                        '--audit[Run in audit mode]' \
                                        '1: :_cuenv_tasks'
                                    ;;
                                exec|e)
                                    _arguments \
                                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                                        '--audit[Run in audit mode]' \
                                        '1: :_command_names'
                                    ;;
                            esac
                            ;;
                    esac
                    ;;
                run)
                    _arguments \
                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                        '--audit[Run in audit mode]' \
                        '1: :_cuenv_tasks'
                    ;;
                exec)
                    _arguments \
                        '(-e --env)'{-e,--env}'[Environment to use]:environment:_cuenv_environments' \
                        '(-c --capability)'{-c,--capability}'[Capabilities to enable]:capability:_cuenv_capabilities' \
                        '--audit[Run in audit mode]' \
                        '1: :_command_names'
                    ;;
                completion)
                    _arguments \
                        '1: :(bash zsh fish powershell)'
                    ;;
            esac
            ;;
    esac
}

_cuenv_commands() {
    local commands
    commands=(
        'task:Manage and execute tasks'
        't:Manage and execute tasks (alias)'
        'init:Initialize a new env.cue file'
        'status:Display current environment status'
        'allow:Allow cuenv in a directory'
        'deny:Deny cuenv in a directory'
        'run:Run a task or command with the environment'
        'exec:Execute a command with the environment'
        'export:Export environment variables for the current directory'
        'dump:Dump complete environment'
        'prune:Prune stale state'
        'cache:Cache management commands'
        'shell:Shell integration commands'
        'completion:Generate shell completion scripts'
        'help:Print help information'
    )
    _describe 'commands' commands
}

_cuenv_task_commands() {
    local commands
    commands=(
        'list:List available tasks'
        'l:List available tasks (alias)'
        'run:Run a task with the loaded environment'
        'r:Run a task (alias)'
        'exec:Execute a command with the loaded environment'
        'e:Execute a command (alias)'
    )
    _describe 'task commands' commands
}

_cuenv_tasks() {
    local tasks
    tasks=($(cuenv _complete_tasks 2>/dev/null))
    _describe 'tasks' tasks
}

_cuenv_environments() {
    local envs
    envs=($(cuenv _complete_environments 2>/dev/null))
    if [[ -z "$envs" ]]; then
        envs=(development staging production)
    fi
    _describe 'environments' envs
}

_cuenv_capabilities() {
    local caps
    caps=(network filesystem secrets)
    _describe 'capabilities' caps
}

_cuenv "$@"
"#;
    print!("{script}");
    Ok(())
}
