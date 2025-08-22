use crate::events::{TracingEvent, TracingLevel};
use chrono::Timelike;
use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Scrollbar, ScrollbarOrientation, ScrollbarState, Wrap},
    Frame,
};
use std::collections::VecDeque;

pub struct TracingPane {
    events: VecDeque<TracingEvent>,
    max_events: usize,
    scroll_offset: u16,
    auto_scroll: bool,
    visible: bool,
    min_level: TracingLevel,
    target_filter: Option<String>,
}

impl TracingPane {
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
            max_events: 1000, // Ring buffer size
            scroll_offset: 0,
            auto_scroll: true,
            visible: true,
            min_level: TracingLevel::Info,
            target_filter: None,
        }
    }

    pub fn add_event(&mut self, event: TracingEvent) {
        // Only add events that meet the minimum level filter
        if self.should_include_event(&event) {
            if self.events.len() >= self.max_events {
                self.events.pop_front();
            }
            self.events.push_back(event);

            if self.auto_scroll {
                self.scroll_to_bottom();
            }
        }
    }

    pub fn render(&mut self, frame: &mut Frame<'_>, area: Rect) {
        self.render_with_focus(frame, area, false);
    }

    pub fn render_with_focus(&mut self, frame: &mut Frame<'_>, area: Rect, focused: bool) {
        if !self.visible {
            return;
        }

        let title = format!(" System Tracing ({} events) ", self.events.len());

        let border_style = if focused {
            Style::default().fg(Color::Green)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let mut block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(border_style);

        if self.auto_scroll {
            block = block.title_bottom(" [Auto-scroll] ");
        }

        let level_indicator = format!(" [{}+] ", self.min_level.prefix());
        block = block.title_bottom(level_indicator);

        let inner_area = block.inner(area);
        frame.render_widget(block, area);

        if self.events.is_empty() {
            let empty_msg = Paragraph::new("No tracing events")
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true });
            frame.render_widget(empty_msg, inner_area);
            return;
        }

        self.render_events(frame, inner_area);
    }

    fn render_events(&mut self, frame: &mut Frame<'_>, area: Rect) {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([Constraint::Min(0), Constraint::Length(1)])
            .split(area);

        let events_area = chunks[0];
        let scrollbar_area = chunks[1];

        // Filter and format events
        let filtered_events: Vec<&TracingEvent> = self
            .events
            .iter()
            .filter(|event| self.should_include_event(event))
            .collect();

        let event_lines = self.format_event_lines(&filtered_events);

        if event_lines.is_empty() {
            let empty_msg = Paragraph::new("No events match current filters")
                .style(Style::default().fg(Color::DarkGray))
                .wrap(Wrap { trim: true });
            frame.render_widget(empty_msg, events_area);
            return;
        }

        // Calculate visible range
        let visible_height = events_area.height as usize;
        let total_lines = event_lines.len();

        let scroll_offset = if self.auto_scroll {
            total_lines.saturating_sub(visible_height) as u16
        } else {
            self.scroll_offset
        };

        let start_idx = (scroll_offset as usize).min(total_lines.saturating_sub(1));
        let end_idx = (start_idx + visible_height).min(total_lines);

        let visible_lines = if start_idx < end_idx {
            event_lines[start_idx..end_idx].to_vec()
        } else {
            Vec::new()
        };

        let paragraph = Paragraph::new(visible_lines).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, events_area);

        // Render scrollbar if needed
        if total_lines > visible_height {
            let mut scrollbar_state = ScrollbarState::default()
                .content_length(total_lines.saturating_sub(visible_height))
                .position(scroll_offset as usize);

            let scrollbar = Scrollbar::default()
                .orientation(ScrollbarOrientation::VerticalRight)
                .begin_symbol(Some("↑"))
                .end_symbol(Some("↓"));

            frame.render_stateful_widget(scrollbar, scrollbar_area, &mut scrollbar_state);
        }
    }

    fn should_include_event(&self, event: &TracingEvent) -> bool {
        // Check minimum level
        let level_order = |level: &TracingLevel| -> u8 {
            match level {
                TracingLevel::Trace => 0,
                TracingLevel::Debug => 1,
                TracingLevel::Info => 2,
                TracingLevel::Warn => 3,
                TracingLevel::Error => 4,
            }
        };

        if level_order(&event.level) < level_order(&self.min_level) {
            return false;
        }

        // Check target filter
        if let Some(ref filter) = self.target_filter {
            if !event.target.contains(filter) && !event.message.contains(filter) {
                return false;
            }
        }

        true
    }

    fn format_event_lines<'a>(&self, events: &[&'a TracingEvent]) -> Vec<Line<'a>> {
        events
            .iter()
            .map(|event| self.format_event_line(event))
            .collect()
    }

    fn format_event_line<'a>(&self, event: &'a TracingEvent) -> Line<'a> {
        let timestamp = format!(
            "{:02}:{:02}:{:02}.{:03}",
            event.timestamp.hour(),
            event.timestamp.minute(),
            event.timestamp.second(),
            event.timestamp.timestamp_subsec_millis()
        );

        let mut spans = vec![
            Span::styled(
                format!("[{}] ", timestamp),
                Style::default().fg(Color::DarkGray),
            ),
            Span::styled(event.level.icon(), event.level.style()),
            Span::styled(format!(" {} ", event.level.prefix()), event.level.style()),
        ];

        // Add target if it's not too long
        if event.target.len() <= 20 {
            spans.push(Span::styled(
                format!("[{}] ", event.target),
                Style::default().fg(Color::Cyan),
            ));
        }

        spans.push(Span::styled(
            &event.message,
            Style::default().fg(Color::White),
        ));

        // Add fields if any
        if !event.fields.is_empty() {
            let fields_str = event
                .fields
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join(" ");

            spans.push(Span::styled(
                format!(" {}", fields_str),
                Style::default().fg(Color::DarkGray),
            ));
        }

        Line::from(spans)
    }

    pub fn scroll_up(&mut self, amount: u16) {
        self.auto_scroll = false;
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let total_lines = self.events.len();
        let max_scroll = total_lines.saturating_sub(10) as u16;

        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);

        // If we've scrolled to the bottom, re-enable auto-scroll
        if self.scroll_offset >= max_scroll {
            self.auto_scroll = true;
        } else {
            self.auto_scroll = false;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
        let total_lines = self.events.len();
        self.scroll_offset = total_lines.saturating_sub(10) as u16;
    }

    pub fn toggle_visibility(&mut self) {
        self.visible = !self.visible;
    }

    pub fn toggle_auto_scroll(&mut self) {
        self.auto_scroll = !self.auto_scroll;
        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn set_min_level(&mut self, level: TracingLevel) {
        self.min_level = level;
    }

    pub fn set_target_filter(&mut self, filter: Option<String>) {
        self.target_filter = filter;
    }

    pub fn clear_events(&mut self) {
        self.events.clear();
        self.scroll_offset = 0;
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn is_auto_scroll_enabled(&self) -> bool {
        self.auto_scroll
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
    }
}

impl Default for TracingPane {
    fn default() -> Self {
        Self::new()
    }
}
