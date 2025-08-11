use super::events::EventHandler;
use super::focus::FocusedPane;
use super::input::InputHandler;
use super::render::Renderer;
use crate::{
    components::{EnvPane, FocusPane, MiniMap},
    event_bus::{EventBus, EventSubscriber},
    terminal::{InputEvent, TerminalManager},
};
use cuenv_task::executor::TaskExecutor;
use std::collections::HashMap;
use tracing::info;

pub struct TuiApp {
    pub(super) terminal: TerminalManager,
    pub(super) minimap: MiniMap,
    pub(super) focus_pane: FocusPane,
    pub(super) env_pane: EnvPane,
    pub(super) event_subscriber: EventSubscriber,
    pub(super) running: bool,
    pub(super) focused_pane: FocusedPane,
    pub(super) task_executor: TaskExecutor,
}

impl TuiApp {
    pub async fn new(
        event_bus: EventBus,
        task_executor: TaskExecutor,
    ) -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let terminal = TerminalManager::new()?;

        let registry = event_bus.registry().clone();
        let minimap = MiniMap::new(registry.clone());
        let focus_pane = FocusPane::new(registry);

        // Start with no environment variables - will be updated when a task is selected
        let env_pane = EnvPane::new(HashMap::new());
        let event_subscriber = event_bus.subscribe();

        Ok(Self {
            terminal,
            minimap,
            focus_pane,
            env_pane,
            event_subscriber,
            running: true,
            focused_pane: FocusedPane::MiniMap,
            task_executor,
        })
    }

    pub async fn run(&mut self) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        info!("Starting TUI application");

        // Initial render
        self.minimap.build_tree_lines().await;

        // Initialize env pane with first selected task
        if let Some(task) = self.minimap.get_selected_task() {
            let task_clone = task.clone();
            self.update_env_pane_for_task(&task_clone);
        }

        self.render()?;

        // Timer for updating task info when needed
        let mut update_timer = tokio::time::interval(std::time::Duration::from_millis(50));
        update_timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

        // Keep TUI alive until explicit quit, even if tasks error, as per PRD
        while self.running {
            tokio::select! {
                // Handle task events
                Some(event) = self.event_subscriber.recv() => {
                    // If a terminal error occurs, keep UI open; allow user to inspect and press 'q'
                    self.handle_task_event(event).await;
                    self.render()?;
                }

                // Check if we need to update task info
                _ = update_timer.tick() => {
                    if self.focus_pane.needs_task_info_update() {
                        self.focus_pane.update_task_info().await;
                        self.render()?;
                    }
                }

                // Handle input events
                Some(input) = self.terminal.next_event() => {
                    match input {
                        InputEvent::Key(key) => {
                            if TerminalManager::should_quit(&key) {
                                self.running = false;
                            } else {
                                self.handle_key_event(key).await;
                                self.render()?;
                            }
                        }
                        InputEvent::Resize => {
                            self.render()?;
                        }
                        InputEvent::Tick => {
                            // Keep ticking to allow redraw while idle
                        }
                    }
                }

                // No events: small idle sleep to respect CPU budget
                else => {
                    // Yield to avoid busy loop; keep under ~2% of a single core
                    tokio::time::sleep(std::time::Duration::from_millis(16)).await;
                }
            }
        }

        info!("TUI application shutting down");
        Ok(())
    }

    pub(super) fn update_env_pane_for_task(&mut self, task_name: &str) {
        let filtered_vars = self.task_executor.get_task_env_vars(task_name);
        self.env_pane = EnvPane::new(filtered_vars);
    }
}