use cuenv_core::Result;

pub fn generate_completion(shell: &str) -> Result<()> {
    match shell.to_lowercase().as_str() {
        "bash" => generate_bash_completion(),
        "zsh" => generate_zsh_completion(),
        "fish" => generate_fish_completion(),
        "powershell" | "pwsh" => generate_powershell_completion(),
        "elvish" => generate_elvish_completion(),
        _ => {
            eprintln!("Unsupported shell: {shell}");
            eprintln!("Supported shells: bash, zsh, fish, powershell, elvish");
            std::process::exit(1);
        }
    }
}

fn generate_bash_completion() -> Result<()> {
    let script = r#"
_cuenv_completion() {
    local cur prev opts
    COMPREPLY=()
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"

    # Main commands
    local commands="task init status allow deny run exec export dump prune cache shell completion help"
    
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
    
    # Handle task completion for 'run' command (legacy)
    if [[ "${COMP_WORDS[1]}" == "run" ]] && [[ ${COMP_CWORD} == 2 ]]; then
        COMPREPLY=($(compgen -W "$(cuenv _complete_tasks 2>/dev/null)" -- ${cur}))
        return 0
    fi
    
    # Complete flags for all commands
    local flags="-h --help -V --version -e --env -c --capability --audit"
    COMPREPLY=($(compgen -W "${flags}" -- ${cur}))
}

complete -F _cuenv_completion cuenv
"#;
    print!("{script}");
    Ok(())
}

fn generate_zsh_completion() -> Result<()> {
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

fn generate_fish_completion() -> Result<()> {
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
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "init" -d "Initialize a new env.cue file"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "status" -d "Display current environment status"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "allow" -d "Allow cuenv in a directory"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "deny" -d "Deny cuenv in a directory"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "run" -d "Run a task with the environment"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "exec" -d "Execute a command with the environment"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "export" -d "Export environment variables"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "dump" -d "Dump complete environment"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "prune" -d "Prune stale state"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "cache" -d "Cache management"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "shell" -d "Shell integration"
complete -f -c cuenv -n "test (count (commandline -opc)) = 1" -a "completion" -d "Generate completion scripts"

# Task subcommands
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "list l" -d "List available tasks"
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "run r" -d "Run a task"
complete -f -c cuenv -n "__fish_cuenv_using_command task; or __fish_cuenv_using_command t" -a "exec e" -d "Execute a command"

# Task names for task run
complete -f -c cuenv -n "__fish_cuenv_using_task_subcommand run; or __fish_cuenv_using_task_subcommand r" -a "(cuenv _complete_tasks 2>/dev/null)"

# Task names for legacy run command
complete -f -c cuenv -n "__fish_cuenv_using_command run" -a "(cuenv _complete_tasks 2>/dev/null)"

# Global options
complete -f -c cuenv -s h -l help -d "Print help information"
complete -f -c cuenv -s V -l version -d "Print version information"
complete -f -c cuenv -s e -l env -d "Environment to use"
complete -f -c cuenv -s c -l capability -d "Capabilities to enable"
complete -f -c cuenv -l audit -d "Run in audit mode"
"#;
    print!("{script}");
    Ok(())
}

fn generate_powershell_completion() -> Result<()> {
    let script = r#"
using namespace System.Management.Automation
using namespace System.Management.Automation.Language

Register-ArgumentCompleter -Native -CommandName 'cuenv' -ScriptBlock {
    param($wordToComplete, $commandAst, $cursorPosition)

    $commandElements = $commandAst.CommandElements
    $command = @(
        'cuenv'
        for ($i = 1; $i -lt $commandElements.Count; $i++) {
            $element = $commandElements[$i]
            if ($element -isnot [StringConstantExpressionAst] -or
                $element.StringConstantType -ne [StringConstantType]::BareWord -or
                $element.Value.StartsWith('-')) {
                break
        }
        $element.Value
    }) -join ';'

    $completions = @(switch ($command) {
        'cuenv' {
            [CompletionResult]::new('task', 'task', [CompletionResultType]::ParameterValue, 'Manage and execute tasks')
            [CompletionResult]::new('t', 't', [CompletionResultType]::ParameterValue, 'Manage and execute tasks (alias)')
            [CompletionResult]::new('init', 'init', [CompletionResultType]::ParameterValue, 'Initialize a new env.cue file')
            [CompletionResult]::new('status', 'status', [CompletionResultType]::ParameterValue, 'Display current environment status')
            [CompletionResult]::new('allow', 'allow', [CompletionResultType]::ParameterValue, 'Allow cuenv in a directory')
            [CompletionResult]::new('deny', 'deny', [CompletionResultType]::ParameterValue, 'Deny cuenv in a directory')
            [CompletionResult]::new('run', 'run', [CompletionResultType]::ParameterValue, 'Run a task with the environment')
            [CompletionResult]::new('exec', 'exec', [CompletionResultType]::ParameterValue, 'Execute a command with the environment')
            [CompletionResult]::new('export', 'export', [CompletionResultType]::ParameterValue, 'Export environment variables')
            [CompletionResult]::new('dump', 'dump', [CompletionResultType]::ParameterValue, 'Dump complete environment')
            [CompletionResult]::new('prune', 'prune', [CompletionResultType]::ParameterValue, 'Prune stale state')
            [CompletionResult]::new('cache', 'cache', [CompletionResultType]::ParameterValue, 'Cache management')
            [CompletionResult]::new('shell', 'shell', [CompletionResultType]::ParameterValue, 'Shell integration')
            [CompletionResult]::new('completion', 'completion', [CompletionResultType]::ParameterValue, 'Generate completion scripts')
            break
        }
        'cuenv;task' {
            [CompletionResult]::new('list', 'list', [CompletionResultType]::ParameterValue, 'List available tasks')
            [CompletionResult]::new('l', 'l', [CompletionResultType]::ParameterValue, 'List available tasks (alias)')
            [CompletionResult]::new('run', 'run', [CompletionResultType]::ParameterValue, 'Run a task')
            [CompletionResult]::new('r', 'r', [CompletionResultType]::ParameterValue, 'Run a task (alias)')
            [CompletionResult]::new('exec', 'exec', [CompletionResultType]::ParameterValue, 'Execute a command')
            [CompletionResult]::new('e', 'e', [CompletionResultType]::ParameterValue, 'Execute a command (alias)')
            break
        }
        'cuenv;task;run' {
            & cuenv _complete_tasks 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
        'cuenv;run' {
            & cuenv _complete_tasks 2>$null | ForEach-Object {
                [CompletionResult]::new($_, $_, [CompletionResultType]::ParameterValue, $_)
            }
            break
        }
    })

    $completions.Where{ $_.CompletionText -like "$wordToComplete*" } |
        Sort-Object -Property ListItemText
}
"#;
    print!("{script}");
    Ok(())
}

fn generate_elvish_completion() -> Result<()> {
    let script = r#"
use builtin;
use str;

set edit:completion:arg-completer[cuenv] = {|@words|
    fn spaces {|n|
        builtin:repeat $n ' ' | str:join ''
    }
    fn cand {|text desc|
        edit:complex-candidate $text &display=$text' '(spaces (- 14 (wcswidth $text)))$desc
    }
    var command = 'cuenv'
    for word $words[1..-1] {
        if (str:has-prefix $word '-') {
            break
        }
        set command = $command';'$word
    }
    var completions = [
        &'cuenv'= {
            cand task 'Manage and execute tasks'
            cand t 'Manage and execute tasks (alias)'
            cand init 'Initialize a new env.cue file'
            cand status 'Display current environment status'
            cand allow 'Allow cuenv in a directory'
            cand deny 'Deny cuenv in a directory'
            cand run 'Run a task with the environment'
            cand exec 'Execute a command with the environment'
            cand export 'Export environment variables'
            cand dump 'Dump complete environment'
            cand prune 'Prune stale state'
            cand cache 'Cache management'
            cand shell 'Shell integration'
            cand completion 'Generate completion scripts'
        }
        &'cuenv;task'= {
            cand list 'List available tasks'
            cand l 'List available tasks (alias)'
            cand run 'Run a task'
            cand r 'Run a task (alias)'
            cand exec 'Execute a command'
            cand e 'Execute a command (alias)'
        }
        &'cuenv;task;run'= {
            cuenv _complete_tasks 2>/dev/null | each {|task|
                cand $task 'Task'
            }
        }
        &'cuenv;run'= {
            cuenv _complete_tasks 2>/dev/null | each {|task|
                cand $task 'Task'
            }
        }
    ]
    $completions[$command]
}
"#;
    print!("{script}");
    Ok(())
}
