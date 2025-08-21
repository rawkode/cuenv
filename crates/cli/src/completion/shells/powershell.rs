//! PowerShell completion generator

use cuenv_core::Result;

/// Generate PowerShell completion script
pub fn generate() -> Result<()> {
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
    tracing::info!("{script}");
    Ok(())
}
