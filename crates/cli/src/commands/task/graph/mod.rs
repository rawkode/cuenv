pub mod formats;

use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;

/// Supported graph output formats
#[derive(Debug, Clone, PartialEq)]
pub enum GraphFormat {
    Tree,
    Json,
    Dot,
    Mermaid,
    D2,
}

impl GraphFormat {
    /// Parse format from string, defaulting to Tree if None or invalid
    pub fn from_option(format: Option<String>) -> Self {
        match format.as_deref() {
            Some("tree") | None => GraphFormat::Tree,
            Some("json") => GraphFormat::Json,
            Some("dot") => GraphFormat::Dot,
            Some("mermaid") => GraphFormat::Mermaid,
            Some("d2") => GraphFormat::D2,
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
    fn format_graph(&self, dag: &UnifiedTaskDAG, task_name: &str) -> Result<String>;
}

/// Main function to format and display a graph
pub fn display_formatted_graph(
    dag: &UnifiedTaskDAG,
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
        GraphFormat::Dot => {
            let formatter = formats::dot::DotFormatter::new();
            formatter.format_graph(dag, task_name)?
        }
        GraphFormat::Mermaid => {
            let formatter = formats::mermaid::MermaidFormatter::new();
            formatter.format_graph(dag, task_name)?
        }
        GraphFormat::D2 => {
            let formatter = formats::d2::D2Formatter::new();
            formatter.format_graph(dag, task_name)?
        }
    };

    print!("{output}");
    Ok(())
}
