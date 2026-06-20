use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use crate::tui::state::{TuiSnapshot, LogLevel};

pub struct LogPanel<'a> {
    snapshot: &'a TuiSnapshot,
    scroll_start: usize,
}

impl<'a> LogPanel<'a> {
    pub fn new(snapshot: &'a TuiSnapshot, scroll_start: usize) -> Self {
        Self { snapshot, scroll_start }
    }
}

impl<'a> Widget for LogPanel<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let log_count = self.snapshot.logs.len();
        let title = format!(" ALIGNMENT LOGS [{}] ", log_count);

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Magenta))
            .style(Style::default().fg(Color::Reset));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 1 || inner.width < 10 {
            return;
        }

        let visible_lines = inner.height as usize;
        let logs = &self.snapshot.logs;

        let start_idx = self.scroll_start.min(logs.len().saturating_sub(1));
        let start_idx = start_idx.min(logs.len().saturating_sub(visible_lines.max(1)));

        let display_logs: Vec<_> = logs.iter().skip(start_idx).take(visible_lines).collect();

        for (line_idx, log_entry) in display_logs.iter().enumerate() {
            let y = inner.y + line_idx as u16;
            if y >= inner.bottom() {
                break;
            }

            let (level_str, level_style) = match log_entry.level {
                LogLevel::Info => ("INFO", Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD)),
                LogLevel::Warn => ("WARN", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
                LogLevel::Error => ("ERROR", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            };

            let prefix = format!("[{}] ", level_str);
            let max_msg_len = inner.width as usize - prefix.len();
            let message: String = log_entry.message.chars().take(max_msg_len).collect();

            let mut x = inner.x;
            for ch in prefix.chars() {
                if x < inner.right() {
                    buf.get_mut(x, y).set_char(ch).set_style(level_style);
                    x += 1;
                }
            }

            let msg_style = match log_entry.level {
                LogLevel::Info => Style::default().fg(Color::Gray),
                LogLevel::Warn => Style::default().fg(Color::LightYellow),
                LogLevel::Error => Style::default().fg(Color::LightRed),
            };

            for ch in message.chars() {
                if x < inner.right() {
                    buf.get_mut(x, y).set_char(ch).set_style(msg_style);
                    x += 1;
                }
            }
        }

        if logs.len() > visible_lines {
            let scroll_indicator = format!(" ↓{}/{} ", logs.len() - visible_lines - start_idx, logs.len());
            let indicator_x = inner.right().saturating_sub(scroll_indicator.len() as u16);
            for (i, ch) in scroll_indicator.chars().enumerate() {
                let x = indicator_x + i as u16;
                if x < inner.right() {
                    buf.get_mut(x, inner.bottom().saturating_sub(1))
                        .set_char(ch)
                        .set_fg(Color::DarkGray);
                }
            }
        }
    }
}
