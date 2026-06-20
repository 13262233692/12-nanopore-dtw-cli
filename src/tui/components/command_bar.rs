use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use crate::tui::state::PipelineStatus;

pub struct CommandBar<'a> {
    status: PipelineStatus,
    hint: &'a str,
}

impl<'a> CommandBar<'a> {
    pub fn new(status: PipelineStatus) -> Self {
        Self {
            status,
            hint: "",
        }
    }

    pub fn with_hint(mut self, hint: &'a str) -> Self {
        self.hint = hint;
        self
    }
}

impl<'a> Widget for CommandBar<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Blue))
            .style(Style::default().bg(Color::Black));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 {
            return;
        }

        let bar_y = inner.y;
        let mut x = inner.x + 1;

        self.render_key_label(x, bar_y, "Space", "Pause/Resume", buf, &mut x);
        x += 2;
        self.render_key_label(x, bar_y, "↑/↓", "Scroll Logs", buf, &mut x);
        x += 2;
        self.render_key_label(x, bar_y, "Q", "Quit", buf, &mut x);

        if !self.hint.is_empty() {
            let hint_x = inner.right().saturating_sub(self.hint.len() as u16 + 2);
            for (i, ch) in self.hint.chars().enumerate() {
                let hx = hint_x + i as u16;
                if hx < inner.right() - 1 {
                    buf.get_mut(hx, bar_y)
                        .set_char(ch)
                        .set_fg(Color::DarkGray);
                }
            }
        }

        let status_text = match self.status {
            PipelineStatus::Running => "▶ ACTIVE ",
            PipelineStatus::Paused => "⏸ PAUSED ",
            PipelineStatus::Completed => "✓ DONE ",
            PipelineStatus::Failed => "✗ ERROR ",
        };
        let status_style = match self.status {
            PipelineStatus::Running => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD).bg(Color::Black),
            PipelineStatus::Paused => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD).bg(Color::Black),
            PipelineStatus::Completed => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD).bg(Color::Black),
            PipelineStatus::Failed => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD).bg(Color::Black),
        };

        if inner.width > 50 {
            let status_x = inner.x + inner.width / 2 - status_text.len() as u16 / 2;
            for (i, ch) in status_text.chars().enumerate() {
                let sx = status_x + i as u16;
                if sx < inner.right() - 1 {
                    buf.get_mut(sx, bar_y).set_char(ch).set_style(status_style);
                }
            }
        }
    }
}

impl<'a> CommandBar<'a> {
    fn render_key_label(
        &self,
        _start_x: u16,
        y: u16,
        key: &str,
        label: &str,
        buf: &mut Buffer,
        x_cursor: &mut u16,
    ) {
        let key_style = Style::default()
            .fg(Color::Black)
            .bg(Color::Cyan)
            .add_modifier(Modifier::BOLD);

        for ch in key.chars() {
            let cx = *x_cursor;
            buf.get_mut(cx, y).set_char(ch).set_style(key_style);
            *x_cursor += 1;
        }

        let label_with_space = format!(" {}", label);
        for ch in label_with_space.chars() {
            let cx = *x_cursor;
            buf.get_mut(cx, y).set_char(ch).set_fg(Color::LightCyan);
            *x_cursor += 1;
        }
    }
}
