use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, SystemTime};

use crossterm::event::{self, Event as CrosstermEvent, KeyCode, KeyEventKind};
use crossterm::terminal::{self, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{cursor, execute};
use notify::{RecursiveMode, Watcher};

enum Event {
    Key(crossterm::event::KeyEvent),
    Resize,
    FsChange,
    Tick,
}

pub fn run(cwd: &str) -> anyhow::Result<()> {
    let ccm_dir = PathBuf::from(cwd).join(".ccm");
    let plans_dir = ccm_dir.join("plans");

    // Enable raw mode + alternate screen
    terminal::enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        run_inner(&ccm_dir, &plans_dir)
    }));

    // Cleanup always runs, even on panic
    let _ = terminal::disable_raw_mode();
    let _ = execute!(io::stdout(), LeaveAlternateScreen);

    match result {
        Ok(inner_result) => inner_result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn run_inner(ccm_dir: &Path, plans_dir: &Path) -> anyhow::Result<()> {
    let (tx, rx) = mpsc::channel();

    // Crossterm event reader thread
    let tx_input = tx.clone();
    thread::spawn(move || loop {
        if event::poll(Duration::from_millis(100)).unwrap_or(false) {
            if let Ok(evt) = event::read() {
                let mapped = match evt {
                    CrosstermEvent::Key(k) => Some(Event::Key(k)),
                    CrosstermEvent::Resize(_, _) => Some(Event::Resize),
                    _ => None,
                };
                if let Some(e) = mapped {
                    if tx_input.send(e).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // File watcher on .ccm/ directory (recursive, so plans/ creation is detected)
    let tx_watch = tx.clone();
    let watch_target = ccm_dir.to_path_buf();
    let mut watcher =
        notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
            if let Ok(event) = res {
                let is_relevant = event
                    .paths
                    .iter()
                    .any(|p| p.extension().is_some_and(|ext| ext == "md") || p.is_dir());
                if is_relevant {
                    let _ = tx_watch.send(Event::FsChange);
                }
            }
        })?;

    // Create .ccm dir if it doesn't exist so watcher can start
    std::fs::create_dir_all(ccm_dir)?;
    watcher.watch(&watch_target, RecursiveMode::Recursive)?;

    // Tick timer (3 second fallback)
    thread::spawn(move || loop {
        thread::sleep(Duration::from_secs(3));
        if tx.send(Event::Tick).is_err() {
            break;
        }
    });

    // State
    let mut last_path: Option<PathBuf> = None;
    let mut last_mtime: Option<SystemTime> = None;
    let mut content = String::new();
    let mut scroll_offset: usize = 0;

    // Initial display
    match find_newest_md(plans_dir) {
        Some(path) => {
            last_mtime = path.metadata().ok().and_then(|m| m.modified().ok());
            content = std::fs::read_to_string(&path).unwrap_or_default();
            render(&mut io::stdout(), &path, &content, scroll_offset)?;
            last_path = Some(path);
        }
        None => {
            display_waiting(plans_dir)?;
        }
    }

    // `watcher` must remain in scope to keep the file watcher alive

    while let Ok(event) = rx.recv() {
        match event {
            Event::Key(key) => {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                let lines: Vec<&str> = content.lines().collect();
                let total = lines.len();
                let (_, rows) = terminal::size().unwrap_or((80, 24));
                let content_height = (rows as usize).saturating_sub(2);
                let max_offset = total.saturating_sub(content_height);

                match key.code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('j') | KeyCode::Down => {
                        scroll_offset = (scroll_offset + 1).min(max_offset);
                    }
                    KeyCode::Char('k') | KeyCode::Up => {
                        scroll_offset = scroll_offset.saturating_sub(1);
                    }
                    KeyCode::PageDown | KeyCode::Char(' ') => {
                        scroll_offset = (scroll_offset + content_height).min(max_offset);
                    }
                    KeyCode::PageUp => {
                        scroll_offset = scroll_offset.saturating_sub(content_height);
                    }
                    KeyCode::Home | KeyCode::Char('g') => {
                        scroll_offset = 0;
                    }
                    KeyCode::End => {
                        scroll_offset = max_offset;
                    }
                    KeyCode::Char('G') => {
                        scroll_offset = max_offset;
                    }
                    _ => continue,
                }

                if let Some(ref path) = last_path {
                    render(&mut io::stdout(), path, &content, scroll_offset)?;
                }
            }
            Event::Resize => {
                if let Some(ref path) = last_path {
                    render(&mut io::stdout(), path, &content, scroll_offset)?;
                } else {
                    display_waiting(plans_dir)?;
                }
            }
            Event::FsChange | Event::Tick => {
                let newest = find_newest_md(plans_dir);
                let current_mtime = newest
                    .as_ref()
                    .and_then(|p| p.metadata().ok())
                    .and_then(|m| m.modified().ok());

                let file_changed = match (&last_path, &newest) {
                    (None, None) => false,
                    (Some(_), None) | (None, Some(_)) => true,
                    (Some(a), Some(b)) => a != b || last_mtime != current_mtime,
                };

                if file_changed {
                    let path_changed = match (&last_path, &newest) {
                        (Some(a), Some(b)) => a != b,
                        _ => true,
                    };
                    // Reset scroll on file switch
                    if path_changed {
                        scroll_offset = 0;
                    }

                    match &newest {
                        Some(path) => {
                            content = std::fs::read_to_string(path).unwrap_or_default();
                            // Clamp scroll_offset after content change
                            let total = content.lines().count();
                            let (_, rows) = terminal::size().unwrap_or((80, 24));
                            let ch = (rows as usize).saturating_sub(2);
                            scroll_offset = scroll_offset.min(total.saturating_sub(ch));
                            render(&mut io::stdout(), path, &content, scroll_offset)?;
                        }
                        None => {
                            content.clear();
                            display_waiting(plans_dir)?;
                        }
                    }
                    last_path = newest;
                    last_mtime = current_mtime;
                }
            }
        }
    }

    Ok(())
}

fn render(
    stdout: &mut io::Stdout,
    path: &Path,
    content: &str,
    scroll_offset: usize,
) -> anyhow::Result<()> {
    let filename = path.file_name().unwrap_or_default().to_string_lossy();
    let (_, rows) = terminal::size().unwrap_or((80, 24));
    let content_height = (rows as usize).saturating_sub(2);
    let lines: Vec<&str> = content.lines().collect();
    let total = lines.len();

    // Clamp scroll_offset
    let offset = scroll_offset.min(total.saturating_sub(content_height));

    let end_line = (offset + content_height).min(total);
    let start_display = offset + 1; // 1-based
    let end_display = end_line;

    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    // Header with scroll position
    if total == 0 {
        writeln!(stdout, "\x1b[1;36m=== {} === [empty]\x1b[0m", filename)?;
    } else {
        writeln!(
            stdout,
            "\x1b[1;36m=== {} === [L{}-L{}/{}]\x1b[0m",
            filename, start_display, end_display, total
        )?;
    }
    // Separator line
    writeln!(stdout)?;

    // Content
    for line in lines.iter().skip(offset).take(content_height) {
        writeln!(stdout, "{}", line)?;
    }
    stdout.flush()?;

    Ok(())
}

/// Find the newest .md file in the plans directory by modification time.
pub fn find_newest_md(plans_dir: &Path) -> Option<PathBuf> {
    let entries = std::fs::read_dir(plans_dir).ok()?;

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .is_some_and(|ext| ext == "md")
        })
        .max_by_key(|e| {
            e.metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH)
        })
        .map(|e| e.path())
}

fn display_waiting(plans_dir: &Path) -> anyhow::Result<()> {
    let mut stdout = io::stdout();
    execute!(
        stdout,
        terminal::Clear(terminal::ClearType::All),
        cursor::MoveTo(0, 0)
    )?;

    writeln!(stdout, "\x1b[2mWaiting for plans...\x1b[0m")?;
    writeln!(stdout)?;
    writeln!(
        stdout,
        "\x1b[2mExpecting .md files in:\n  {}\x1b[0m",
        plans_dir.display()
    )?;
    stdout.flush()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn find_newest_md_empty_dir() {
        let tmp = TempDir::new().unwrap();
        let plans = tmp.path().join("plans");
        fs::create_dir_all(&plans).unwrap();
        assert!(find_newest_md(&plans).is_none());
    }

    #[test]
    fn find_newest_md_single_file() {
        let tmp = TempDir::new().unwrap();
        let plans = tmp.path().join("plans");
        fs::create_dir_all(&plans).unwrap();
        let file = plans.join("init.md");
        fs::write(&file, "# Plan").unwrap();
        let result = find_newest_md(&plans);
        assert_eq!(result, Some(file));
    }

    #[test]
    fn find_newest_md_multiple_files() {
        let tmp = TempDir::new().unwrap();
        let plans = tmp.path().join("plans");
        fs::create_dir_all(&plans).unwrap();

        let old = plans.join("old.md");
        fs::write(&old, "old").unwrap();

        // Ensure different mtime
        std::thread::sleep(Duration::from_millis(50));

        let new = plans.join("new.md");
        fs::write(&new, "new").unwrap();

        let result = find_newest_md(&plans).unwrap();
        assert_eq!(result, new);
    }

    #[test]
    fn find_newest_md_ignores_non_md() {
        let tmp = TempDir::new().unwrap();
        let plans = tmp.path().join("plans");
        fs::create_dir_all(&plans).unwrap();
        fs::write(plans.join("notes.txt"), "text").unwrap();
        fs::write(plans.join("data.json"), "{}").unwrap();
        assert!(find_newest_md(&plans).is_none());
    }

    #[test]
    fn find_newest_md_nonexistent_dir() {
        let tmp = TempDir::new().unwrap();
        let plans = tmp.path().join("nonexistent");
        assert!(find_newest_md(&plans).is_none());
    }
}
