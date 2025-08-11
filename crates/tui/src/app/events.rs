use super::core::TuiApp;
use crate::events::TaskEvent;
use tracing::debug;

pub trait EventHandler {
    async fn handle_task_event(&mut self, event: TaskEvent);
    async fn select_task_in_minimap(&mut self, task_name: &str);
}

impl EventHandler for TuiApp {
    async fn handle_task_event(&mut self, event: TaskEvent) {
        debug!("Handling task event: {:?}", event);

        match &event {
            TaskEvent::Started { task_name, .. } => {
                // Always rebuild tree when a task starts
                self.minimap.build_tree_lines().await;

                // Auto-select the running task to show immediate feedback
                self.select_task_in_minimap(task_name).await;
            }
            TaskEvent::Completed { .. }
            | TaskEvent::Failed { .. }
            | TaskEvent::Cancelled { .. } => {
                // Rebuild tree when task states change
                self.minimap.build_tree_lines().await;

                // On first failure, jump to the first error to reduce time-to-first-error
                if matches!(event, TaskEvent::Failed { .. }) && self.minimap.jump_to_first_error() {
                    if let Some(task) = self.minimap.get_selected_task() {
                        self.focus_pane.set_task(task.clone());
                    }
                }

                // Update focus pane if it's showing the affected task
                if let Some(current_task) = self.focus_pane.get_current_task() {
                    match &event {
                        TaskEvent::Started { task_name, .. }
                        | TaskEvent::Progress { task_name, .. }
                        | TaskEvent::Completed { task_name, .. }
                        | TaskEvent::Failed { task_name, .. }
                        | TaskEvent::Cancelled { task_name } => {
                            if current_task == task_name {
                                self.focus_pane.update_task_info().await;
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }

        // Keep focus pane synced to currently selected node
        if let Some(selected) = self.minimap.get_selected_task() {
            let selected_clone = selected.clone();
            self.focus_pane.set_task(selected_clone.clone());
            self.update_env_pane_for_task(&selected_clone);
        }
    }

    async fn select_task_in_minimap(&mut self, task_name: &str) {
        // Go to top first
        self.minimap.jump_to_top();

        // Search for the task
        let mut found = false;
        for _ in 0..1000 {
            // Safety limit
            if let Some(selected) = self.minimap.get_selected_task() {
                if selected == task_name {
                    found = true;
                    break;
                }
            }
            self.minimap.select_next();
        }

        if found {
            self.focus_pane.set_task(task_name.to_string());
            self.update_env_pane_for_task(task_name);
        }
    }
}
