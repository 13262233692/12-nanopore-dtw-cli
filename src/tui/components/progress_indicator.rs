use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    widgets::{Block, Borders, Widget},
};
use crate::tui::state::TuiSnapshot;
use crate::tui::state::PipelineStatus;

pub struct ProgressIndicator<'a> {
    snapshot: &'a TuiSnapshot,
}

impl<'a> ProgressIndicator<'a> {
    pub fn new(snapshot: &'a TuiSnapshot) -> Self {
        Self { snapshot }
    }
}

impl<'a> Widget for ProgressIndicator<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let title = format!(" THREAD POOL & QUEUE ");

        let block = Block::default()
            .borders(Borders::ALL)
            .title(title)
            .border_style(Style::default().fg(Color::Yellow))
            .style(Style::default().fg(Color::Reset));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.height < 6 || inner.width < 20 {
            return;
        }

        let bar_width = (inner.width - 12) as usize;

        let queue_pct = self.snapshot.queue_percent();
        self.render_progress_bar(
            inner.x,
            inner.y,
            inner.width,
            "Queue Depth",
            self.snapshot.queue_depth,
            self.snapshot.max_queue_depth,
            queue_pct,
            bar_width,
            buf,
        );

        let progress_pct = self.snapshot.progress_percent();
        let total = self.snapshot.total_reads;
        let processed = self.snapshot.processed_reads;
        self.render_progress_bar(
            inner.x,
            inner.y + 2,
            inner.width,
            "Progress",
            processed,
            total,
            progress_pct,
            bar_width,
            buf,
        );

        let active_pct = if self.snapshot.file_count > 0 {
            (self.snapshot.active_files as f64 / self.snapshot.file_count as f64) * 100.0
        } else {
            0.0
        };
        self.render_progress_bar(
            inner.x,
            inner.y + 4,
            inner.width,
            "Active Files",
            self.snapshot.active_files,
            self.snapshot.file_count,
            active_pct,
            bar_width,
            buf,
        );

        if inner.height > 8 {
            let stats_y = inner.y + 6;
            let rps_text = format!("  Reads/s: {:.1}", self.snapshot.reads_per_second());
            let bps_text = format!("  Bases/s: {:.0}", self.snapshot.total_bases as f64 / self.snapshot.elapsed.as_secs_f64().max(0.001));
            let elapsed_text = format!("  Elapsed: {}", format_duration(&self.snapshot.elapsed));

            for (i, ch) in rps_text.chars().enumerate() {
                let x = inner.x + i as u16;
                if x < inner.right() && stats_y < inner.bottom() {
                    buf.get_mut(x, stats_y).set_char(ch).set_fg(Color::Cyan);
                }
            }
            for (i, ch) in bps_text.chars().enumerate() {
                let x = inner.x + i as u16;
                if x < inner.right() && stats_y + 1 < inner.bottom() {
                    buf.get_mut(x, stats_y + 1).set_char(ch).set_fg(Color::Green);
                }
            }
            for (i, ch) in elapsed_text.chars().enumerate() {
                let x = inner.x + i as u16;
                if x < inner.right() && stats_y + 2 < inner.bottom() {
                    buf.get_mut(x, stats_y + 2).set_char(ch).set_fg(Color::Yellow);
                }
            }
        }

        let status_text = match self.snapshot.status {
            PipelineStatus::Running => "● RUNNING",
            PipelineStatus::Paused => "⏸ PAUSED",
            PipelineStatus::Completed => "✓ COMPLETE",
            PipelineStatus::Failed => "✗ FAILED",
        };
        let status_style = match self.snapshot.status {
            PipelineStatus::Running => Style::default().fg(Color::Green).add_modifier(Modifier::BOLD),
            PipelineStatus::Paused => Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
            PipelineStatus::Completed => Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            PipelineStatus::Failed => Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        };

        let status_y = inner.bottom().saturating_sub(2);
        for (i, ch) in status_text.chars().enumerate() {
            let x = inner.x + 2 + i as u16;
            if x < inner.right() - 2 {
                buf.get_mut(x, status_y).set_char(ch).set_style(status_style);
            }
        }
    }
}

impl<'a> ProgressIndicator<'a> {
    fn render_progress_bar(
        &self,
        x: u16,
        y: u16,
        width: u16,
        label: &str,
        current: usize,
        max: usize,
        percent: f64,
        bar_width: usize,
        buf: &mut Buffer,
    ) {
        let label_text = format!("{}: ", label);
        let mut pos = x;

        for ch in label_text.chars() {
            if pos < x + width {
                buf.get_mut(pos, y).set_char(ch).set_fg(Color::LightYellow);
                pos += 1;
            }
        }

        let bar_start = pos;
        let filled = ((percent / 100.0) * bar_width as f64) as usize;
        let filled = filled.min(bar_width);

        for i in 0..bar_width {
            let bar_x = bar_start + i as u16;
            if bar_x >= x + width {
                break;
            }

            let cell = buf.get_mut(bar_x, y);
            if i < filled {
                let ratio = i as f64 / bar_width as f64;
                if ratio > 0.8 {
                    cell.set_symbol("█").set_fg(Color::Red);
                } else if ratio > 0.5 {
                    cell.set_symbol("█").set_fg(Color::Yellow);
                } else {
                    cell.set_symbol("█").set_fg(Color::Green);
                }
            } else {
                cell.set_symbol("░").set_fg(Color::DarkGray);
            }
        }

        let value_text = format!(" {}/{}", current, max);
        let value_start = bar_start + bar_width as u16 + 1;
        for (i, ch) in value_text.chars().enumerate() {
            let val_x = value_start + i as u16;
            if val_x < x + width {
                buf.get_mut(val_x, y).set_char(ch).set_fg(Color::White);
            }
        }
    }
}

fn format_duration(d: &std::time::Duration) -> String {
    let secs = d.as_secs();
    let hours = secs / 3600;
    let minutes = (secs % 3600) / 60;
    let seconds = secs % 60;
    if hours > 0 {
        format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
    } else {
        format!("{:02}:{:02}", minutes, seconds)
    }
}
