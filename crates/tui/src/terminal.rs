use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEvent, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;
use tokio::sync::mpsc;

pub enum InputEvent {
    Key(KeyEvent),
    Resize,
    Tick,
}

pub struct TerminalManager {
    terminal: Terminal<CrosstermBackend<io::Stderr>>,
    event_rx: mpsc::UnboundedReceiver<InputEvent>,
    _event_task: tokio::task::JoinHandle<()>,
}

impl TerminalManager {
    pub fn new() -> io::Result<Self> {
        // Setup terminal
        enable_raw_mode()?;
        let mut stderr = io::stderr();
        execute!(stderr, EnterAlternateScreen, EnableMouseCapture)?;

        let backend = CrosstermBackend::new(stderr);
        let terminal = Terminal::new(backend)?;

        // Setup event handling
        let (event_tx, event_rx) = mpsc::unbounded_channel();
        let event_task = tokio::spawn(Self::event_loop(event_tx));

        Ok(Self {
            terminal,
            event_rx,
            _event_task: event_task,
        })
    }

    async fn event_loop(tx: mpsc::UnboundedSender<InputEvent>) {
        let mut tick_interval = tokio::time::interval(Duration::from_millis(100));
        tick_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        loop {
            // Use tokio::select! to handle multiple async operations without blocking
            tokio::select! {
                // Poll for terminal events in a non-blocking way
                _ = tokio::time::sleep(Duration::from_millis(10)) => {
                    // Run the blocking poll/read operations in a blocking thread pool
                    // This prevents them from blocking the tokio runtime
                    let has_event = tokio::task::spawn_blocking(|| {
                        event::poll(Duration::from_millis(0)).unwrap_or(false)
                    }).await.unwrap_or(false);

                    if has_event {
                        let event_result = tokio::task::spawn_blocking(|| {
                            event::read()
                        }).await;

                        if let Ok(Ok(event)) = event_result {
                            let input_event = match event {
                                Event::Key(key) => Some(InputEvent::Key(key)),
                                Event::Resize(_, _) => Some(InputEvent::Resize),
                                _ => None,
                            };

                            if let Some(evt) = input_event {
                                if tx.send(evt).is_err() {
                                    break;
                                }
                            }
                        }
                    }
                }

                // Handle tick events
                _ = tick_interval.tick() => {
                    if tx.send(InputEvent::Tick).is_err() {
                        break;
                    }
                }
            }
        }
    }

    pub fn terminal(&mut self) -> &mut Terminal<CrosstermBackend<io::Stderr>> {
        &mut self.terminal
    }

    pub async fn next_event(&mut self) -> Option<InputEvent> {
        self.event_rx.recv().await
    }

    pub fn should_quit(key: &KeyEvent) -> bool {
        matches!(
            key,
            KeyEvent {
                code: KeyCode::Char('q'),
                modifiers: KeyModifiers::NONE,
                ..
            } | KeyEvent {
                code: KeyCode::Char('c'),
                modifiers: KeyModifiers::CONTROL,
                ..
            }
        )
    }
}

impl Drop for TerminalManager {
    fn drop(&mut self) {
        // Restore terminal
        let _ = disable_raw_mode();
        let _ = execute!(
            self.terminal.backend_mut(),
            LeaveAlternateScreen,
            DisableMouseCapture
        );
        let _ = self.terminal.show_cursor();
    }
}
