use super::core::TuiApp;
use super::focus::FocusedPane;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub trait InputHandler {
    async fn handle_key_event(&mut self, key: KeyEvent);
}

impl InputHandler for TuiApp {
    async fn handle_key_event(&mut self, key: KeyEvent) {
        match key.code {
            // Pane switching
            KeyCode::Tab => {
                self.focused_pane = self.focused_pane.next();
            }

            // Navigation
            KeyCode::Up | KeyCode::Char('k') => match self.focused_pane {
                FocusedPane::MiniMap => {
                    self.minimap.select_previous();
                    if let Some(task) = self.minimap.get_selected_task() {
                        let task_clone = task.clone();
                        self.focus_pane.set_task(task_clone.clone());
                        self.update_env_pane_for_task(&task_clone);
                    }
                }
                FocusedPane::TaskDetails => {
                    self.focus_pane.scroll_up(1);
                }
                FocusedPane::Environment => {
                    self.env_pane.select_previous();
                }
            },
            KeyCode::Down | KeyCode::Char('j') => match self.focused_pane {
                FocusedPane::MiniMap => {
                    self.minimap.select_next();
                    if let Some(task) = self.minimap.get_selected_task() {
                        let task_clone = task.clone();
                        self.focus_pane.set_task(task_clone.clone());
                        self.update_env_pane_for_task(&task_clone);
                    }
                }
                FocusedPane::TaskDetails => {
                    self.focus_pane.scroll_down(1);
                }
                FocusedPane::Environment => {
                    self.env_pane.select_next();
                }
            },

            // Tree expansion
            KeyCode::Left | KeyCode::Char('h') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Right | KeyCode::Char('l') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Enter | KeyCode::Char(' ') => {
                self.minimap.toggle_expand();
                self.minimap.build_tree_lines().await;
            }

            // Scrolling
            KeyCode::PageUp => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus_pane.scroll_up(10);
                } else {
                    self.minimap.scroll_up(10);
                }
            }
            KeyCode::PageDown => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    self.focus_pane.scroll_down(10);
                } else {
                    self.minimap.scroll_down(10);
                }
            }

            // Jump commands (PRD: g/G operate on mini-map selection)
            KeyCode::Char('g') => {
                if key.modifiers.contains(KeyModifiers::SHIFT) {
                    // Shift+g behaves like 'G'
                    self.minimap.jump_to_bottom();
                } else {
                    self.minimap.jump_to_top();
                }
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }
            KeyCode::Char('G') => {
                self.minimap.jump_to_bottom();
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }
            KeyCode::Char('E') => {
                self.minimap.jump_to_first_error();
                if let Some(task) = self.minimap.get_selected_task() {
                    let task_clone = task.clone();
                    self.focus_pane.set_task(task_clone.clone());
                    self.update_env_pane_for_task(&task_clone);
                }
            }

            // Tree manipulation
            KeyCode::Char('*') => {
                self.minimap.expand_all();
                self.minimap.build_tree_lines().await;
            }
            KeyCode::Char('/') => {
                self.minimap.collapse_all();
                self.minimap.build_tree_lines().await;
            }

            // Focus pane controls
            KeyCode::Char('a') => {
                self.focus_pane.toggle_auto_scroll();
            }

            _ => {}
        }
    }
}