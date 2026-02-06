mod app;
mod event;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, KeyCode, KeyEventKind, MouseEventKind,
};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use crate::config::Config;

use self::app::App;
use self::event::{Event, EventHandler};

pub fn run(session_name: &str, config: &Config) -> anyhow::Result<()> {
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    crossterm::execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_event_loop(&mut terminal, session_name, config)
    }));

    // Cleanup always runs, even on panic
    let _ = terminal::disable_raw_mode();
    let _ = crossterm::execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    );
    let _ = terminal.show_cursor();

    match result {
        Ok(inner_result) => inner_result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn run_event_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    session_name: &str,
    config: &Config,
) -> anyhow::Result<()> {
    let events = EventHandler::new(Duration::from_secs(config.tui.tick_interval_secs))?;
    let mut app = App::new(session_name, &config.wezterm.binary);
    let mut last_area_width: u16 = 0;

    loop {
        terminal.draw(|f| {
            last_area_width = f.area().width;
            ui::draw(f, &app);
        })?;

        match events.next()? {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                // If in confirm-delete mode, handle y/n
                if app.confirm_delete.is_some() {
                    match key.code {
                        KeyCode::Char('y') | KeyCode::Char('Y') => app.confirm_delete_yes(),
                        _ => app.confirm_delete_no(),
                    }
                    continue;
                }

                match key.code {
                    KeyCode::Char('q') => {
                        app.should_quit = true;
                    }
                    KeyCode::Char('j') | KeyCode::Down => app.move_down(),
                    KeyCode::Char('k') | KeyCode::Up => app.move_up(),
                    KeyCode::Enter => app.switch_to_selected(),
                    KeyCode::Char('d') => app.request_delete(),
                    KeyCode::Char('r') => {
                        app.reconcile();
                        app.refresh_state();
                    }
                    _ => {}
                }

                // Clear status on any key press
                app.status_message = None;
            }
            Event::Mouse(mouse) => {
                if let MouseEventKind::Down(_) = mouse.kind {
                    app.select_by_click(mouse.row, last_area_width);
                }
            }
            Event::Resize => {}
            Event::StateChanged => {
                app.refresh_state();
            }
            Event::Tick => {
                app.reconcile();
            }
        }

        if app.should_quit {
            break;
        }
    }

    Ok(())
}
