use super::core::TuiApp;
use super::focus::FocusedPane;
use crate::components::{EnvPane, FocusPane, MiniMap};
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Color, Style},
    widgets::{Block, Borders},
    Frame,
};

pub trait Renderer {
    fn render(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}

impl Renderer for TuiApp {
    fn render(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let minimap = &mut self.minimap;
        let focus_pane = &mut self.focus_pane;
        let env_pane = &mut self.env_pane;
        let focused = self.focused_pane;

        self.terminal.terminal().draw(|f| {
            draw_ui(f, minimap, focus_pane, env_pane, focused);
        })?;
        Ok(())
    }
}

fn draw_ui(
    frame: &mut Frame<'_>,
    minimap: &mut MiniMap,
    focus_pane: &mut FocusPane,
    env_pane: &mut EnvPane,
    focused: FocusedPane,
) {
    // Main layout: split screen horizontally
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(40), // Mini-map
            Constraint::Percentage(60), // Right side (focus pane & env)
        ])
        .split(frame.area());

    // Draw mini-map with border highlight if focused
    let minimap_border_style = if focused == FocusedPane::MiniMap {
        Style::default().fg(Color::Cyan)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    let minimap_block = Block::default()
        .borders(Borders::RIGHT)
        .border_style(minimap_border_style);
    let minimap_area = minimap_block.inner(chunks[0]);
    frame.render_widget(minimap_block, chunks[0]);
    minimap.render(frame, minimap_area);

    // Split the right side vertically for focus pane and env pane
    let right_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage(60), // Focus pane (task details & logs)
            Constraint::Percentage(40), // Environment pane
        ])
        .split(chunks[1]);

    // Draw focus pane
    focus_pane.render(frame, right_chunks[0]);

    // Draw environment pane with border highlight if focused
    env_pane.render(frame, right_chunks[1]);

    // Draw help bar at the bottom
    draw_help_bar(frame);
}

fn draw_help_bar(frame: &mut Frame<'_>) {
    let help_text = " Tab: Switch Pane │ ↑↓/jk: Navigate │ ←→/hl/Space: Expand │ E: First Error │ g/G: Top/Bottom │ a: Auto-scroll │ q: Quit ";
    let help_bar = Block::default()
        .title(help_text)
        .title_style(Style::default().fg(Color::DarkGray))
        .borders(Borders::TOP);

    let help_area = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(1)])
        .split(frame.area())[1];

    frame.render_widget(help_bar, help_area);
}
