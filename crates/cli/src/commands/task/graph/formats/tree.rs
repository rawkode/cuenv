use crate::commands::task::graph::{CharSet, GraphFormatter};
use cuenv_core::Result;
use cuenv_task::TaskDAG;

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
                vertical: "│ ",
            },
            CharSet::Ascii => TreeSymbols {
                branch: "|-",
                last_branch: "`-",
                vertical: "| ",
            },
        }
    }
}

struct TreeSymbols {
    branch: &'static str,
    last_branch: &'static str,
    vertical: &'static str,
}

impl GraphFormatter for TreeFormatter {
    fn format_graph(&self, dag: &TaskDAG, selected_task: &str) -> Result<String> {
        let mut output = String::new();
        let symbols = self.get_symbols();

        // Show the selected task at the top
        output.push_str(&format!("🎯 {}\n", selected_task));

        // Build tree by showing what the selected task depends on
        self.build_dependency_tree_down(dag, selected_task, "", true, &symbols, &mut output);

        Ok(output)
    }
}

impl TreeFormatter {
    /// Build dependency tree by traversing dependencies (what this task depends on)  
    fn build_dependency_tree_down(
        &self,
        dag: &TaskDAG,
        task_name: &str,
        prefix: &str,
        is_last: bool,
        symbols: &TreeSymbols,
        output: &mut String,
    ) {
        if let Some(dependencies) = dag.get_task_dependencies(task_name) {
            let dep_count = dependencies.len();

            for (i, dep_name) in dependencies.iter().enumerate() {
                let is_last_dep = i == dep_count - 1;

                let current_prefix = if is_last {
                    format!(
                        "{}{} ",
                        prefix,
                        if is_last_dep {
                            symbols.last_branch
                        } else {
                            symbols.branch
                        }
                    )
                } else {
                    format!(
                        "{}{} ",
                        prefix,
                        if is_last_dep {
                            symbols.last_branch
                        } else {
                            symbols.branch
                        }
                    )
                };

                // Use the task name as-is (preserve original format)
                let formatted_name = dep_name.clone();

                output.push_str(&format!("{}{}\n", current_prefix, formatted_name));

                // Recursively build tree for this dependency
                let next_prefix = if is_last {
                    format!("{}  ", prefix)
                } else {
                    format!("{}{} ", prefix, symbols.vertical)
                };

                self.build_dependency_tree_down(
                    dag,
                    dep_name,
                    &next_prefix,
                    is_last_dep,
                    symbols,
                    output,
                );
            }
        }
    }
}
