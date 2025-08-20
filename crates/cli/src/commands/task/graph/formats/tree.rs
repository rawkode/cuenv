use crate::commands::task::graph::{CharSet, GraphFormatter};
use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;

pub struct TreeFormatter {
    charset: CharSet,
}

impl TreeFormatter {
    pub fn new(charset: CharSet) -> Self {
        Self { charset }
    }

    fn get_symbols(&self) -> TreeSymbols {
        match self.charset {
            CharSet::Unicode => TreeSymbols {
                branch: "├─",
                last_branch: "└─",
                vertical: "│",
                space: "  ",
            },
            CharSet::Ascii => TreeSymbols {
                branch: "+--",
                last_branch: "`--",
                vertical: "|",
                space: "   ",
            },
        }
    }
}

struct TreeSymbols {
    branch: &'static str,
    last_branch: &'static str,
    vertical: &'static str,
    space: &'static str,
}

impl GraphFormatter for TreeFormatter {
    fn format_graph(&self, dag: &UnifiedTaskDAG, root_name: &str) -> Result<String> {
        let mut output = String::new();
        let symbols = self.get_symbols();

        output.push_str(&format!("{root_name}\n"));

        // Get the execution levels (topologically sorted)
        match dag.get_execution_levels() {
            Ok(levels) => {
                if levels.is_empty() {
                    output.push_str(&format!("{} No dependencies\n", symbols.last_branch));
                    return Ok(output);
                }

                // Show execution order
                for (level_num, level_tasks) in levels.iter().enumerate() {
                    let is_last_level = level_num == levels.len() - 1;
                    let _level_symbol = if is_last_level {
                        symbols.last_branch
                    } else {
                        symbols.branch
                    };

                    for (i, task) in level_tasks.iter().enumerate() {
                        let is_last_in_level = i == level_tasks.len() - 1;
                        let task_symbol = if is_last_in_level && is_last_level {
                            symbols.last_branch
                        } else {
                            symbols.branch
                        };

                        output.push_str(&format!("{task_symbol} {task}\n"));

                        // Show dependencies for this task
                        if let Some(deps) = dag.get_task_dependencies(task) {
                            if !deps.is_empty() {
                                let dep_prefix = if is_last_in_level && is_last_level {
                                    symbols.space
                                } else {
                                    &format!("{} ", symbols.vertical)
                                };
                                output.push_str(&format!(
                                    "{}   (depends on: {})\n",
                                    dep_prefix,
                                    deps.join(", ")
                                ));
                            }
                        }
                    }
                }
            }
            Err(e) => {
                output.push_str(&format!(
                    "{} Error building execution graph: {}\n",
                    symbols.last_branch, e
                ));
            }
        }

        Ok(output)
    }
}
