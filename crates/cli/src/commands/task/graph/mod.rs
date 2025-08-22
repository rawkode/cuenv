pub mod formats;

use cuenv_core::Result;

/// Supported graph output formats
#[derive(Debug, Clone, PartialEq)]
pub enum GraphFormat {
    Tree,
    Json,
}

impl GraphFormat {
    /// Parse format from string, defaulting to Tree if None or invalid
    pub fn from_option(format: Option<String>) -> Self {
        match format.as_deref() {
            Some("tree") | None => GraphFormat::Tree,
            Some("json") => GraphFormat::Json,
            Some(_) => GraphFormat::Tree, // Default for unknown formats
        }
    }
}

/// Character set options for tree format
#[derive(Debug, Clone, PartialEq)]
pub enum CharSet {
    Unicode,
    Ascii,
}

impl CharSet {
    pub fn from_str(charset: &str) -> Self {
        match charset {
            "ascii" => CharSet::Ascii,
            _ => CharSet::Unicode,
        }
    }
}

/// Trait for formatting task dependency graphs  
pub trait GraphFormatter {
    /// Format the given DAG with the specified task name
    fn format_graph(&self, dag: &cuenv_task::TaskDAG, task_name: &str) -> Result<String>;
}

/// Function to display DAGs
pub fn display_formatted_graph(
    dag: &cuenv_task::TaskDAG,
    task_name: &str,
    format: GraphFormat,
    charset: CharSet,
) -> Result<()> {
    let output = match format {
        GraphFormat::Tree => {
            let formatter = formats::tree::TreeFormatter::new(charset);
            formatter.format_graph(dag, task_name)?
        }
        GraphFormat::Json => {
            let formatter = formats::json::JsonFormatter::new();
            formatter.format_graph(dag, task_name)?
        }
    };

    print!("{output}");
    Ok(())
}
