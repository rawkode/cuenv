use super::MiniMap;
use ratatui::{
    style::{Color, Style},
    text::{Line, Span},
};

impl MiniMap {
    pub fn scroll_up(&mut self, amount: u16) {
        self.scroll_offset = self.scroll_offset.saturating_sub(amount);
    }

    pub fn scroll_down(&mut self, amount: u16) {
        let max_scroll = (self.visible_lines.len() as u16).saturating_sub(20);
        self.scroll_offset = (self.scroll_offset + amount).min(max_scroll);
    }

    pub fn scroll_left(&mut self, amount: u16) {
        self.horizontal_scroll = self.horizontal_scroll.saturating_sub(amount);
    }

    pub fn scroll_right(&mut self, amount: u16, area_width: u16) {
        if self.max_line_width > area_width {
            let max_scroll = self.max_line_width.saturating_sub(area_width);
            self.horizontal_scroll = (self.horizontal_scroll + amount).min(max_scroll);
        }
    }

    pub fn ensure_selected_visible_horizontally(&mut self, area_width: u16) {
        if let Some(selected) = &self.selected_task {
            if let Some(line) = self.visible_lines.iter().find(|l| l.task_name == *selected) {
                let line_start = line.prefix.len() as u16;
                let line_end = line_start + line.task_name.len() as u16 + 4; // icon and spacing

                if line_start < self.horizontal_scroll {
                    self.horizontal_scroll = line_start;
                } else if line_end > self.horizontal_scroll + area_width {
                    self.horizontal_scroll = line_end.saturating_sub(area_width);
                }
            }
        }
    }

    pub(crate) fn apply_horizontal_scroll(
        &self,
        line: Line,
        visible_width: usize,
    ) -> Line<'static> {
        let mut total_width = 0;
        let mut visible_spans = Vec::new();
        let scroll_offset = self.horizontal_scroll as usize;

        for span in line.spans {
            let span_text = span.content.to_string();
            let span_width = unicode_width::UnicodeWidthStr::width(span_text.as_str());

            if total_width + span_width <= scroll_offset {
                total_width += span_width;
                continue;
            }

            if total_width >= scroll_offset + visible_width {
                break;
            }

            let start_in_span = scroll_offset.saturating_sub(total_width);
            let visible_start = start_in_span.min(span_text.len());
            let remaining_width = visible_width
                - visible_spans
                    .iter()
                    .map(|s: &Span| unicode_width::UnicodeWidthStr::width(s.content.as_ref()))
                    .sum::<usize>();

            let visible_text = span_text
                .chars()
                .skip(visible_start)
                .take(remaining_width)
                .collect::<String>();

            if !visible_text.is_empty() {
                visible_spans.push(Span::styled(visible_text, span.style));
            }

            total_width += span_width;
        }

        if self.horizontal_scroll > 0 && visible_spans.is_empty() {
            visible_spans.push(Span::styled("←", Style::default().fg(Color::DarkGray)));
        }

        Line::from(visible_spans)
    }
}
