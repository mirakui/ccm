use std::ffi::OsString;
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use crossterm::event::{self, Event as CrosstermEvent, KeyEvent, MouseEvent};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};

use crate::state;

pub enum Event {
    Key(KeyEvent),
    Mouse(MouseEvent),
    Resize,
    StateChanged,
    Tick,
}

pub struct EventHandler {
    rx: mpsc::Receiver<Event>,
    _watcher: RecommendedWatcher,
}

impl EventHandler {
    pub fn new(tick_rate: Duration) -> anyhow::Result<Self> {
        let (tx, rx) = mpsc::channel();

        // Crossterm event reader thread
        let tx_input = tx.clone();
        thread::spawn(move || loop {
            if event::poll(Duration::from_millis(100)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    let mapped = match evt {
                        CrosstermEvent::Key(k) => Some(Event::Key(k)),
                        CrosstermEvent::Mouse(m) => Some(Event::Mouse(m)),
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

        // File watcher
        let tx_watch = tx.clone();
        let state_path = state::state_path()?;
        let watch_dir = state_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("state path has no parent directory"))?
            .to_path_buf();

        // Only send events for state.json changes, not .lock or .tmp files
        let state_filename: OsString = state_path
            .file_name()
            .map(|f| f.to_os_string())
            .unwrap_or_else(|| OsString::from("state.json"));

        std::fs::create_dir_all(&watch_dir)?;

        let mut watcher =
            notify::recommended_watcher(move |res: Result<notify::Event, notify::Error>| {
                if let Ok(event) = res {
                    let is_state_file = event
                        .paths
                        .iter()
                        .any(|p| p.file_name().map(|f| f == state_filename).unwrap_or(false));
                    if is_state_file {
                        let _ = tx_watch.send(Event::StateChanged);
                    }
                }
            })?;
        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

        // Tick timer thread
        let tx_tick = tx;
        thread::spawn(move || loop {
            thread::sleep(tick_rate);
            if tx_tick.send(Event::Tick).is_err() {
                break;
            }
        });

        Ok(Self {
            rx,
            _watcher: watcher,
        })
    }

    pub fn next(&self) -> anyhow::Result<Event> {
        Ok(self.rx.recv()?)
    }
}
