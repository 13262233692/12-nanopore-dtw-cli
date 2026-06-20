use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use crate::tui::state::TuiSnapshot;

pub struct ThroughputChart<'a> {
    snapshot: &'a TuiSnapshot,
}

impl<'a> ThroughputChart<'a> {
    pub fn new(snapshot: &'a TuiSnapshot) -> Self {
        Self { snapshot }
    }
}

impl<'a> Widget for ThroughputChart<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = format!(
            " THROUGHPUT: {:.1} reads/s | {:.1}% complete ",
            self.snapshot.reads_per_second(),
            self.snapshot.progress_percent()
        );

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Cyan))
            .style(Style::default().fg(Color::Reset));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 3 || inner.width < 10 {
            return;
        }

        let history = &self.snapshot.throughput_history;
        let chart_width = inner.width as usize;
        let chart_height = (inner.height - 1) as usize;

        let max_throughput = history.iter().copied().fold(0.0f64, f64::max).max(1.0);

        let samples: Vec<f64> = if history.len() >= chart_width {
            history.iter().rev().take(chart_width).copied().rev().collect()
        } else {
            let mut padded = vec![0.0; chart_width - history.len()];
            padded.extend_from_slice(history);
            padded
        };

        let baseline_y = inner.bottom() - 1;
        let chart_top = inner.y;

        for (x_idx, &value) in samples.iter().enumerate() {
            let x = inner.x + x_idx as u16;
            if x >= inner.right() {
                break;
            }

            let bar_height = ((value / max_throughput) * chart_height as f64) as u16;
            let bar_height = bar_height.min(chart_height as u16).max(1);

            for y_offset in 0..bar_height {
                let y = baseline_y.saturating_sub(y_offset);
                if y < chart_top {
                    break;
                }

                let cell = buf.get_mut(x, y);
                let ratio = y_offset as f64 / chart_height as f64;

                if ratio > 0.8 {
                    cell.set_symbol("█").set_fg(Color::Green);
                } else if ratio > 0.5 {
                    cell.set_symbol("▌").set_fg(Color::Cyan);
                } else if ratio > 0.2 {
                    cell.set_symbol("▄").set_fg(Color::Blue);
                } else {
                    cell.set_symbol("·").set_fg(Color::DarkGray);
                }
            }
        }

        let y_label = format!("{:.0}/s", max_throughput);
        if inner.width > 20 {
            for (i, ch) in y_label.chars().enumerate() {
                if i < inner.width as usize {
                    let cell = buf.get_mut(inner.x + i as u16, chart_top);
                    cell.set_char(ch).set_fg(Color::DarkGray);
                }
            }
        }

        let processed_info = format!(
            " {}/{} reads | {} bases | {} failed ",
            crate::utils::format_number(self.snapshot.processed_reads),
            crate::utils::format_number(self.snapshot.total_reads),
            crate::utils::format_number(self.snapshot.total_bases),
            crate::utils::format_number(self.snapshot.failed_reads),
        );

        if inner.width > 30 {
            let info_x = inner.x + inner.width.saturating_sub(processed_info.len() as u16);
            for (i, ch) in processed_info.chars().enumerate() {
                let x = info_x + i as u16;
                if x < inner.right() {
                    buf.get_mut(x, baseline_y)
                        .set_char(ch)
                        .set_style(Style::default()
                            .fg(Color::Yellow)
                            .add_modifier(Modifier::BOLD));
                }
            }
        }
    }
}
