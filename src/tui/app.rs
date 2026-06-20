use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    Terminal,
};
use std::io::{self, Stdout};
use std::time::{Duration, Instant};

use crate::tui::components::{
    command_bar::CommandBar, log_panel::LogPanel,
    progress_indicator::ProgressIndicator, throughput_chart::ThroughputChart,
};
use crate::tui::state::{PipelineStatus, TuiState, TuiSnapshot};

const TICK_RATE_MS: u64 = 100;

pub struct TuiApp {
    state: TuiState,
    terminal: Terminal<CrosstermBackend<Stdout>>,
    log_scroll_start: usize,
    should_quit: bool,
    auto_scroll: bool,
}

impl TuiApp {
    pub fn new(state: TuiState) -> Result<Self, Box<dyn std::error::Error>> {
        let mut stdout = io::stdout();
        enable_raw_mode()?;
        execute!(stdout, EnterAlternateScreen)?;
        let backend = CrosstermBackend::new(stdout);
        let terminal = Terminal::new(backend)?;

        Ok(Self {
            state,
            terminal,
            log_scroll_start: 0,
            should_quit: false,
            auto_scroll: true,
        })
    }

    pub fn state(&self) -> &TuiState {
        &self.state
    }

    pub fn run<F>(mut self, mut on_quit: F) -> Result<(), Box<dyn std::error::Error>>
    where
        F: FnMut(),
    {
        let mut last_tick = Instant::now();
        let tick_duration = Duration::from_millis(TICK_RATE_MS);

        while !self.should_quit {
            let timeout = tick_duration
                .checked_sub(last_tick.elapsed())
                .unwrap_or_else(|| Duration::from_millis(0));

            if event::poll(timeout)? {
                if let Event::Key(key) = event::read()? {
                    self.handle_key(key);
                }
            }

            if last_tick.elapsed() >= tick_duration {
                self.draw()?;
                last_tick = Instant::now();

                let status = self.state.get_snapshot().status;
                if matches!(status, PipelineStatus::Completed | PipelineStatus::Failed) {
                    self.should_quit = true;
                }
            }
        }

        on_quit();

        Ok(())
    }

    fn handle_key(&mut self, key: KeyEvent) {
        if key.kind != KeyEventKind::Press {
            return;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                self.should_quit = true;
            }
            KeyCode::Char(' ') => {
                self.state.toggle_pause();
            }
            KeyCode::Up | KeyCode::Char('k') | KeyCode::Char('K') => {
                self.scroll_log_up();
            }
            KeyCode::Down | KeyCode::Char('j') | KeyCode::Char('J') => {
                self.scroll_log_down();
            }
            KeyCode::PageUp => {
                for _ in 0..10 {
                    self.scroll_log_up();
                }
            }
            KeyCode::PageDown => {
                for _ in 0..10 {
                    self.scroll_log_down();
                }
            }
            KeyCode::Char('g') => {
                self.scroll_to_top();
            }
            KeyCode::Char('G') => {
                self.scroll_to_bottom();
            }
            KeyCode::Esc => {
                self.should_quit = true;
            }
            _ => {}
        }
    }

    fn scroll_log_up(&mut self) {
        if self.log_scroll_start > 0 {
            self.log_scroll_start -= 1;
            self.auto_scroll = false;
        }
    }

    fn scroll_log_down(&mut self) {
        let snapshot = self.state.get_snapshot();
        let log_count = snapshot.logs.len();
        let visible_lines = 10;
        let max_start = log_count.saturating_sub(visible_lines);
        if self.log_scroll_start < max_start {
            self.log_scroll_start += 1;
        }
        if self.log_scroll_start >= max_start {
            self.auto_scroll = true;
        }
    }

    fn scroll_to_top(&mut self) {
        self.log_scroll_start = 0;
        self.auto_scroll = false;
    }

    fn scroll_to_bottom(&mut self) {
        let snapshot = self.state.get_snapshot();
        let log_count = snapshot.logs.len();
        let visible_lines = 10;
        self.log_scroll_start = log_count.saturating_sub(visible_lines);
        self.auto_scroll = true;
    }

    fn draw(&mut self) -> Result<(), Box<dyn std::error::Error>> {
        let snapshot = self.state.get_snapshot();
        let scroll_start = self.calculate_scroll_start(&snapshot);

        self.terminal.draw(|f| {
            let size = f.size();

            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints(
                    [
                        Constraint::Length(10),
                        Constraint::Min(8),
                        Constraint::Length(3),
                    ]
                    .as_ref(),
                )
                .split(size);

            let chart_area = chunks[0];
            let middle_area = chunks[1];
            let cmd_area = chunks[2];

            let chart = ThroughputChart::new(&snapshot);
            f.render_widget(chart, chart_area);

            let middle_chunks = Layout::default()
                .direction(Direction::Horizontal)
                .constraints(
                    [
                        Constraint::Ratio(3, 5),
                        Constraint::Ratio(2, 5),
                    ]
                    .as_ref(),
                )
                .split(middle_area);

            let log_panel = LogPanel::new(&snapshot, scroll_start);
            f.render_widget(log_panel, middle_chunks[0]);

            let progress = ProgressIndicator::new(&snapshot);
            f.render_widget(progress, middle_chunks[1]);

            let status = snapshot.status;
            let hint = "Press 'q' to quit";
            let cmd_bar = CommandBar::new(status).with_hint(hint);
            f.render_widget(cmd_bar, cmd_area);
        })?;

        Ok(())
    }

    fn calculate_scroll_start(&mut self, snapshot: &TuiSnapshot) -> usize {
        let log_count = snapshot.logs.len();
        
        if self.auto_scroll {
            self.log_scroll_start = log_count.saturating_sub(20);
        }
        
        self.log_scroll_start = self.log_scroll_start.min(log_count.saturating_sub(1));
        self.log_scroll_start
    }
}

impl Drop for TuiApp {
    fn drop(&mut self) {
        let _ = self.terminal.show_cursor();
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen
        );
    }
}

pub fn run_tui_with_pipeline<F>(
    state: TuiState,
    pipeline_fn: F,
) -> Result<(), Box<dyn std::error::Error>>
where
    F: FnOnce(TuiState) -> Result<(), Box<dyn std::error::Error>> + Send + 'static,
{
    let state_clone = state.clone();

    let pipeline_handle = std::thread::Builder::new()
        .name("pipeline-worker".to_string())
        .spawn(move || {
            if let Err(e) = pipeline_fn(state_clone.clone()) {
                state_clone.add_log(crate::tui::state::LogLevel::Error, format!("Pipeline error: {}", e));
                state_clone.set_status(PipelineStatus::Failed);
            } else {
                state_clone.set_status(PipelineStatus::Completed);
            }
        })?;

    let app = TuiApp::new(state)?;
    app.run(|| {})?;

    let _ = pipeline_handle.join();

    Ok(())
}
