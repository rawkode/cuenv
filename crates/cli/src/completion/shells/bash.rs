//! Bash shell completion generator

use cuenv_core::Result;

/// Generate bash completion script
pub fn generate() -> Result<()> {
    let script = r#"
_cuenv_completion() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Main commands
    local commands="task env init discover cache shell completion help"
    
    # Task subcommands
    local task_commands="list run exec"
    
    # Flags that take arguments
    case "${prev}" in
        -e|--env)
            # Complete environment names
            COMPREPLY=($(compgen -W "$(cuenv _complete_environments 2>/dev/null || echo 'development staging production')" -- ${cur}))
            return 0
            ;;
        -c|--capability)
            # Complete capability names
            COMPREPLY=($(compgen -W "network filesystem secrets" -- ${cur}))
            return 0
            ;;
        --shell)
            COMPREPLY=($(compgen -W "bash zsh fish powershell" -- ${cur}))
            return 0
            ;;
    esac
    
    # Complete subcommands
    if [[ ${COMP_CWORD} == 1 ]]; then
        COMPREPLY=($(compgen -W "${commands}" -- ${cur}))
        return 0
    fi
    
    # Handle task subcommand completion
    if [[ "${COMP_WORDS[1]}" == "task" ]] || [[ "${COMP_WORDS[1]}" == "t" ]]; then
        if [[ ${COMP_CWORD} == 2 ]]; then
            COMPREPLY=($(compgen -W "${task_commands}" -- ${cur}))
            return 0
        elif [[ ${COMP_CWORD} == 3 ]] && [[ "${COMP_WORDS[2]}" == "run" || "${COMP_WORDS[2]}" == "r" ]]; then
            COMPREPLY=($(compgen -W "$(cuenv _complete_tasks 2>/dev/null)" -- ${cur}))
            return 0
        fi
    fi
    
    # Complete flags for all commands
    local flags="-h --help -V --version -e --env -c --capability --audit"
    COMPREPLY=($(compgen -W "${flags}" -- ${cur}))
}

complete -F _cuenv_completion cuenv
"#;
    tracing::info!("{script}");
    Ok(())
}
