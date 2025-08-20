use crate::commands::task::graph::GraphFormatter;
use cuenv_core::Result;
use cuenv_task::UnifiedTaskDAG;
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Copy)]
pub struct D2Formatter {}

// D2 Builder structs for proper syntax generation
#[derive(Debug, Clone)]
struct D2Variables {
    vars: HashMap<String, String>,
}

#[derive(Debug, Clone)]
struct D2Style {
    fill: Option<String>,
    stroke: Option<String>,
    stroke_width: Option<u8>,
    stroke_dash: Option<u8>,
    border_radius: Option<u8>,
    font_size: Option<u8>,
    bold: Option<bool>,
}

#[derive(Debug, Clone)]
struct D2Shape {
    id: String,
    label: Option<String>,
    shape: Option<String>,
    icon: Option<String>,
    style: Option<D2Style>,
}

#[derive(Debug, Clone)]
struct D2Container {
    id: String,
    label: Option<String>,
    shapes: Vec<D2Shape>,
    containers: Vec<D2Container>,
    connections: Vec<D2Connection>,
    style: Option<D2Style>,
}

#[derive(Debug, Clone)]
struct D2Connection {
    from: String,
    to: String,
    label: Option<String>,
    arrow_type: D2ArrowType,
    style: Option<D2Style>,
}

#[derive(Debug, Clone)]
enum D2ArrowType {
    Arrow, // ->
    #[allow(dead_code)]
    Line, // --
    #[allow(dead_code)]
    LeftArrow, // <-
    #[allow(dead_code)]
    BiArrow, // <->
}

struct D2Builder {
    variables: D2Variables,
    shapes: Vec<D2Shape>,
    containers: Vec<D2Container>,
    connections: Vec<D2Connection>,
    direction: Option<String>,
    title: Option<String>,
}

impl D2Builder {
    fn new() -> Self {
        Self {
            variables: D2Variables::new(),
            shapes: Vec::new(),
            containers: Vec::new(),
            connections: Vec::new(),
            direction: None,
            title: None,
        }
    }

    fn set_direction(&mut self, direction: &str) -> &mut Self {
        self.direction = Some(direction.to_string());
        self
    }

    fn set_title(&mut self, title: &str) -> &mut Self {
        self.title = Some(title.to_string());
        self
    }

    fn add_variable(&mut self, key: &str, value: &str) -> &mut Self {
        self.variables.add(key, value);
        self
    }

    fn add_shape(&mut self, shape: D2Shape) -> &mut Self {
        self.shapes.push(shape);
        self
    }

    fn add_container(&mut self, container: D2Container) -> &mut Self {
        self.containers.push(container);
        self
    }

    fn add_connection(&mut self, connection: D2Connection) -> &mut Self {
        self.connections.push(connection);
        self
    }

    fn render(&self) -> String {
        let mut output = String::new();

        // Title comment
        if let Some(ref title) = self.title {
            output.push_str(&format!("# {title}\n"));
        }

        // Direction
        if let Some(ref direction) = self.direction {
            output.push_str(&format!("direction: {direction}\n\n"));
        }

        // Variables
        if !self.variables.vars.is_empty() {
            output.push_str(&self.variables.render());
            output.push('\n');
        }

        // Containers
        for container in &self.containers {
            output.push_str(&container.render(0));
            output.push('\n');
        }

        // Individual shapes
        for shape in &self.shapes {
            output.push_str(&shape.render(0));
            output.push('\n');
        }

        // Connections
        for connection in &self.connections {
            output.push_str(&connection.render(0));
        }

        output
    }
}

impl D2Variables {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
        }
    }

    fn add(&mut self, key: &str, value: &str) {
        self.vars.insert(key.to_string(), value.to_string());
    }

    fn render(&self) -> String {
        if self.vars.is_empty() {
            return String::new();
        }

        let mut output = String::from("vars: {\n");
        for (key, value) in &self.vars {
            output.push_str(&format!("  {key}: {value}\n"));
        }
        output.push_str("}\n");
        output
    }
}

impl D2Style {
    fn new() -> Self {
        Self {
            fill: None,
            stroke: None,
            stroke_width: None,
            stroke_dash: None,
            border_radius: None,
            font_size: None,
            bold: None,
        }
    }

    fn fill(mut self, color: &str) -> Self {
        self.fill = Some(color.to_string());
        self
    }

    fn stroke(mut self, color: &str) -> Self {
        self.stroke = Some(color.to_string());
        self
    }

    fn stroke_width(mut self, width: u8) -> Self {
        self.stroke_width = Some(width);
        self
    }

    fn stroke_dash(mut self, dash: u8) -> Self {
        self.stroke_dash = Some(dash);
        self
    }

    fn border_radius(mut self, radius: u8) -> Self {
        self.border_radius = Some(radius);
        self
    }

    fn font_size(mut self, size: u8) -> Self {
        self.font_size = Some(size);
        self
    }

    fn bold(mut self) -> Self {
        self.bold = Some(true);
        self
    }

    fn render(&self, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let mut parts = Vec::new();

        if let Some(ref fill) = self.fill {
            if fill.starts_with("${") {
                parts.push(format!("fill: {fill}"));
            } else {
                parts.push(format!("fill: \"{fill}\""));
            }
        }
        if let Some(ref stroke) = self.stroke {
            if stroke.starts_with("${") {
                parts.push(format!("stroke: {stroke}"));
            } else {
                parts.push(format!("stroke: \"{stroke}\""));
            }
        }
        if let Some(width) = self.stroke_width {
            parts.push(format!("stroke-width: {width}"));
        }
        if let Some(dash) = self.stroke_dash {
            parts.push(format!("stroke-dash: {dash}"));
        }
        if let Some(radius) = self.border_radius {
            parts.push(format!("border-radius: {radius}"));
        }
        if let Some(size) = self.font_size {
            parts.push(format!("font-size: {size}"));
        }
        if Some(true) == self.bold {
            parts.push("bold: true".to_string());
        }

        if parts.is_empty() {
            return String::new();
        }

        if parts.len() == 1 {
            format!(
                "{indent_str}style: {{\n{indent_str}  {}\n{indent_str}}}\n",
                parts[0]
            )
        } else {
            let mut output = format!("{indent_str}style: {{\n");
            for part in parts {
                output.push_str(&format!("{indent_str}  {part}\n"));
            }
            output.push_str(&format!("{indent_str}}}\n"));
            output
        }
    }
}

impl D2Shape {
    fn new(id: &str) -> Self {
        Self {
            id: Self::sanitize_identifier(id),
            label: None,
            shape: None,
            icon: None,
            style: None,
        }
    }

    fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    fn shape(mut self, shape: &str) -> Self {
        self.shape = Some(shape.to_string());
        self
    }

    fn icon(mut self, icon_url: &str) -> Self {
        self.icon = Some(icon_url.to_string());
        self
    }

    fn style(mut self, style: D2Style) -> Self {
        self.style = Some(style);
        self
    }

    fn sanitize_identifier(id: &str) -> String {
        id.replace([':', '.', '-'], "_")
    }

    fn render(&self, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let mut output = String::new();

        // Start shape definition
        output.push_str(&format!("{indent_str}{}", self.id));

        if let Some(ref label) = self.label {
            output.push_str(&format!(": {label}"));
        }

        // Check if we need a block (shape, icon, or style)
        let needs_block = self.shape.is_some() || self.icon.is_some() || self.style.is_some();

        if needs_block {
            output.push_str(" {\n");

            if let Some(ref shape) = self.shape {
                output.push_str(&format!("{indent_str}  shape: {shape}\n"));
            }

            if let Some(ref icon) = self.icon {
                output.push_str(&format!("{indent_str}  icon: {icon}\n"));
            }

            if let Some(ref style) = self.style {
                let style_str = style.render(indent + 1);
                if !style_str.is_empty() {
                    output.push_str(&format!("{indent_str}  {}", style_str.trim_start()));
                }
            }

            output.push_str(&format!("{indent_str}}}\n"));
        } else {
            output.push('\n');
        }

        output
    }
}

impl D2Container {
    fn new(id: &str) -> Self {
        Self {
            id: D2Shape::sanitize_identifier(id),
            label: None,
            shapes: Vec::new(),
            containers: Vec::new(),
            connections: Vec::new(),
            style: None,
        }
    }

    fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    fn style(mut self, style: D2Style) -> Self {
        self.style = Some(style);
        self
    }

    fn add_shape(mut self, shape: D2Shape) -> Self {
        self.shapes.push(shape);
        self
    }

    #[allow(dead_code)]
    fn add_container(mut self, container: D2Container) -> Self {
        self.containers.push(container);
        self
    }

    fn add_connection(mut self, connection: D2Connection) -> Self {
        self.connections.push(connection);
        self
    }

    fn render(&self, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let mut output = String::new();

        // Container header
        output.push_str(&format!("{indent_str}{}", self.id));
        if let Some(ref label) = self.label {
            output.push_str(&format!(": {label}"));
        }
        output.push_str(" {\n");

        // Style
        if let Some(ref style) = self.style {
            let style_str = style.render(indent + 1);
            if !style_str.is_empty() {
                output.push_str(&format!("{}  {}", indent_str, style_str.trim_start()));
            }
        }

        // Nested containers
        for container in &self.containers {
            output.push_str(&container.render(indent + 1));
        }

        // Shapes
        for shape in &self.shapes {
            output.push_str(&shape.render(indent + 1));
        }

        // Internal connections
        for connection in &self.connections {
            output.push_str(&connection.render(indent + 1));
        }

        output.push_str(&format!("{indent_str}}}\n"));
        output
    }
}

impl D2Connection {
    fn new(from: &str, to: &str, arrow_type: D2ArrowType) -> Self {
        Self {
            from: D2Shape::sanitize_identifier(from),
            to: D2Shape::sanitize_identifier(to),
            label: None,
            arrow_type,
            style: None,
        }
    }

    fn label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    fn style(mut self, style: D2Style) -> Self {
        self.style = Some(style);
        self
    }

    fn render(&self, indent: usize) -> String {
        let indent_str = "  ".repeat(indent);
        let arrow = match self.arrow_type {
            D2ArrowType::Arrow => "->",
            D2ArrowType::Line => "--",
            D2ArrowType::LeftArrow => "<-",
            D2ArrowType::BiArrow => "<->",
        };

        let mut output = format!("{indent_str}{} {arrow} {}", self.from, self.to);

        if let Some(ref label) = self.label {
            output.push_str(&format!(": {label}"));
        }

        if let Some(ref style) = self.style {
            output.push_str(" {\n");
            let style_str = style.render(indent + 1);
            if !style_str.is_empty() {
                output.push_str(&format!("{}  {}", indent_str, style_str.trim_start()));
            }
            output.push_str(&format!("{indent_str}}}\n"));
        } else {
            output.push('\n');
        }

        output
    }
}

impl D2Formatter {
    pub fn new() -> Self {
        Self {}
    }

    fn get_task_type(task_id: &str) -> &'static str {
        if task_id.contains("test") {
            "test"
        } else if task_id.contains("build") {
            "build"
        } else if task_id.contains("deploy") {
            "deploy"
        } else if task_id.contains("lint")
            || task_id.contains("format")
            || task_id.contains("check")
        {
            "quality"
        } else if task_id.starts_with("__") && task_id.ends_with("__") {
            "control"
        } else {
            "task"
        }
    }

    fn get_task_icon(task_type: &str) -> &'static str {
        match task_type {
            "test" => "https://icons.terrastruct.com/essentials/074-workflow.svg",
            "build" => "https://icons.terrastruct.com/essentials/092-processing.svg",
            "deploy" => "https://icons.terrastruct.com/essentials/012-upload.svg",
            "quality" => "https://icons.terrastruct.com/essentials/003-check.svg",
            "control" => "https://icons.terrastruct.com/essentials/001-target.svg",
            _ => "https://icons.terrastruct.com/essentials/050-assign.svg",
        }
    }

    fn sanitize_identifier(&self, id: &str) -> String {
        D2Shape::sanitize_identifier(id)
    }

    fn get_display_name(&self, id: &str) -> String {
        if let Some(colon_pos) = id.find(':') {
            id[colon_pos + 1..].to_string()
        } else {
            id.to_string()
        }
    }
}

impl GraphFormatter for D2Formatter {
    fn format_graph(&self, dag: &UnifiedTaskDAG, root_name: &str) -> Result<String> {
        let mut builder = D2Builder::new();

        // Set up the diagram
        builder
            .set_title(&format!("Task execution graph: {root_name}"))
            .set_direction("right");

        // Add comprehensive theme variables
        builder
            .add_variable("task-color", "\"#e1f5fe\"")
            .add_variable("group-color", "\"#fff3e0\"")
            .add_variable("control-color", "\"#f3e5f5\"")
            .add_variable("edge-color", "\"#666\"")
            .add_variable("sequential-color", "\"#4caf50\"")
            .add_variable("parallel-color", "\"#2196f3\"")
            .add_variable("test-fill", "\"#e8f5e8\"")
            .add_variable("test-stroke", "\"#4caf50\"")
            .add_variable("build-fill", "\"#fff3e0\"")
            .add_variable("build-stroke", "\"#ff9800\"")
            .add_variable("deploy-fill", "\"#fce4ec\"")
            .add_variable("deploy-stroke", "\"#e91e63\"")
            .add_variable("quality-fill", "\"#f3e5f5\"")
            .add_variable("quality-stroke", "\"#9c27b0\"")
            .add_variable("default-fill", "\"#f5f5f5\"")
            .add_variable("transparent", "\"transparent\"");

        let flattened = dag.get_flattened_tasks();

        // Group tasks by their group prefix (before ':')
        let mut groups: HashMap<String, Vec<String>> = HashMap::new();
        let mut individual_tasks: Vec<String> = Vec::new();

        for task in flattened {
            if let Some(colon_pos) = task.id.find(':') {
                let group_name = task.id[..colon_pos].to_string();
                groups.entry(group_name).or_default().push(task.id.clone());
            } else {
                individual_tasks.push(task.id.clone());
            }
        }

        // Create sophisticated group containers
        for (group_name, group_tasks) in &groups {
            let group_type = self.determine_group_type(group_name, group_tasks);
            let group_style = self.get_group_style(&group_type);

            let mut container = D2Container::new(group_name)
                .label(&format!("{group_name} ({group_type})"))
                .style(group_style);

            // Add control nodes for sequential groups
            if group_type == "sequential" {
                container = container.add_shape(
                    D2Shape::new("__entry__")
                        .label("Entry")
                        .shape("circle")
                        .icon(Self::get_task_icon("control"))
                        .style(
                            D2Style::new()
                                .fill("${control-color}")
                                .stroke("transparent"),
                        ),
                );
            }

            // Add tasks to the container
            for task in group_tasks {
                let display_name = self.get_display_name(task);
                let task_type = Self::get_task_type(task);
                let task_style = self.get_task_style(task_type);

                container = container.add_shape(
                    D2Shape::new(&self.get_display_name(task))
                        .label(&display_name)
                        .icon(Self::get_task_icon(task_type))
                        .style(task_style),
                );
            }

            // Add control exit node for sequential groups
            if group_type == "sequential" {
                container = container.add_shape(
                    D2Shape::new("__exit__")
                        .label("Exit")
                        .shape("circle")
                        .icon(Self::get_task_icon("control"))
                        .style(
                            D2Style::new()
                                .fill("${control-color}")
                                .stroke("transparent"),
                        ),
                );
            }

            // Add internal connections within the group
            container = self.add_group_connections(container, group_tasks, &group_type);

            builder.add_container(container);
        }

        // Add individual tasks (not in groups), excluding the root task
        for task in &individual_tasks {
            if task != root_name {
                let task_type = Self::get_task_type(task);
                let task_style = self.get_task_style(task_type);

                builder.add_shape(
                    D2Shape::new(task)
                        .label(task)
                        .icon(Self::get_task_icon(task_type))
                        .style(task_style),
                );
            }
        }

        // Add root task with special styling
        let root_type = Self::get_task_type(root_name);
        builder.add_shape(
            D2Shape::new(root_name)
                .label(&format!("🎯 {root_name}"))
                .shape("hexagon")
                .icon(Self::get_task_icon(root_type))
                .style(
                    D2Style::new()
                        .fill("${task-color}")
                        .stroke_width(3)
                        .font_size(16)
                        .bold(),
                ),
        );

        // Add sophisticated connections
        let mut added_edges: HashSet<(String, String)> = HashSet::new();

        for task in flattened {
            for dep in &task.dependencies {
                let connection_label = self.get_connection_label(dep, &task.id);
                let connection_style = self.get_connection_style(dep, &task.id);

                let from_path = self.get_connection_path(dep, &groups);
                let to_path = self.get_connection_path(&task.id, &groups);

                // Avoid duplicate edges
                let edge = (from_path.clone(), to_path.clone());
                if !added_edges.contains(&edge) {
                    builder.add_connection(
                        D2Connection::new(&from_path, &to_path, D2ArrowType::Arrow)
                            .label(&connection_label)
                            .style(connection_style),
                    );
                    added_edges.insert(edge);
                }
            }
        }

        Ok(builder.render())
    }
}

// Helper methods for the enhanced D2 formatter
impl D2Formatter {
    fn determine_group_type(&self, group_name: &str, _tasks: &[String]) -> String {
        // This could be enhanced to detect actual group types from the DAG
        if group_name.contains("test") {
            "parallel".to_string()
        } else {
            "sequential".to_string() // Default for build, deploy, and other types
        }
    }

    fn get_group_style(&self, group_type: &str) -> D2Style {
        match group_type {
            "sequential" => D2Style::new()
                .fill("${group-color}")
                .stroke("${sequential-color}")
                .stroke_dash(3)
                .border_radius(8),
            "parallel" => D2Style::new()
                .fill("${group-color}")
                .stroke("${parallel-color}")
                .stroke_dash(5)
                .border_radius(8),
            _ => D2Style::new()
                .fill("${group-color}")
                .stroke("${edge-color}")
                .stroke_dash(3)
                .border_radius(8),
        }
    }

    fn get_task_style(&self, task_type: &str) -> D2Style {
        match task_type {
            "test" => D2Style::new()
                .fill("${test-fill}")
                .stroke("${test-stroke}")
                .border_radius(4),
            "build" => D2Style::new()
                .fill("${build-fill}")
                .stroke("${build-stroke}")
                .border_radius(4),
            "deploy" => D2Style::new()
                .fill("${deploy-fill}")
                .stroke("${deploy-stroke}")
                .border_radius(4),
            "quality" => D2Style::new()
                .fill("${quality-fill}")
                .stroke("${quality-stroke}")
                .border_radius(4),
            "control" => D2Style::new()
                .fill("${control-color}")
                .stroke("${transparent}"),
            _ => D2Style::new()
                .fill("${default-fill}")
                .stroke("${edge-color}")
                .border_radius(4),
        }
    }

    fn add_group_connections(
        &self,
        mut container: D2Container,
        tasks: &[String],
        group_type: &str,
    ) -> D2Container {
        if group_type == "sequential" && tasks.len() > 1 {
            // Add entry connections
            if let Some(first_task) = tasks.first() {
                let first_display = self.get_display_name(first_task);
                container = container.add_connection(
                    D2Connection::new("__entry__", &first_display, D2ArrowType::Arrow)
                        .label("start")
                        .style(D2Style::new().stroke("${sequential-color}")),
                );
            }

            // Add sequential connections between tasks
            for i in 0..tasks.len() - 1 {
                let from_display = self.get_display_name(&tasks[i]);
                let to_display = self.get_display_name(&tasks[i + 1]);
                container = container.add_connection(
                    D2Connection::new(&from_display, &to_display, D2ArrowType::Arrow)
                        .label("sequential")
                        .style(D2Style::new().stroke("${sequential-color}").stroke_width(2)),
                );
            }

            // Add exit connections
            if let Some(last_task) = tasks.last() {
                let last_display = self.get_display_name(last_task);
                container = container.add_connection(
                    D2Connection::new(&last_display, "__exit__", D2ArrowType::Arrow)
                        .label("complete")
                        .style(D2Style::new().stroke("${sequential-color}")),
                );
            }
        }

        container
    }

    fn get_connection_label(&self, from: &str, to: &str) -> String {
        if from.starts_with("__") && from.ends_with("__") {
            "control flow".to_string()
        } else if from.contains("test") || to.contains("test") {
            "test dependency".to_string()
        } else if from.contains("build") || to.contains("build") {
            "build dependency".to_string()
        } else {
            "depends on".to_string()
        }
    }

    fn get_connection_style(&self, from: &str, _to: &str) -> D2Style {
        let stroke_color = if from.contains("test") {
            "${sequential-color}"
        } else if from.contains("build") {
            "${build-stroke}"
        } else {
            "${edge-color}"
        };

        D2Style::new().stroke(stroke_color).stroke_width(2)
    }

    fn get_connection_path(&self, task_id: &str, groups: &HashMap<String, Vec<String>>) -> String {
        if let Some(colon_pos) = task_id.find(':') {
            let group_name = &task_id[..colon_pos];
            let task_name = &task_id[colon_pos + 1..];

            if groups.contains_key(group_name) {
                if task_name.starts_with("__") && task_name.ends_with("__") {
                    // Control nodes have special handling
                    match task_name {
                        "__start__" => {
                            format!("{}.{}", self.sanitize_identifier(group_name), "__entry__")
                        }
                        "__end__" => {
                            format!("{}.{}", self.sanitize_identifier(group_name), "__exit__")
                        }
                        _ => format!(
                            "{}.{}",
                            self.sanitize_identifier(group_name),
                            self.sanitize_identifier(task_name)
                        ),
                    }
                } else {
                    format!(
                        "{}.{}",
                        self.sanitize_identifier(group_name),
                        self.sanitize_identifier(&self.get_display_name(task_id))
                    )
                }
            } else {
                self.sanitize_identifier(task_id)
            }
        } else {
            self.sanitize_identifier(task_id)
        }
    }
}
