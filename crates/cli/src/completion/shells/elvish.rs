//! Elvish shell completion generator

use cuenv_core::Result;

/// Generate elvish completion script
pub fn generate() -> Result<()> {
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
    tracing::info!("{script}");
    Ok(())
}
