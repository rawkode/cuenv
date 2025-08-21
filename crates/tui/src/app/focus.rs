#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPane {
    TaskHierarchy,
    TaskConfig,
    TaskLogs,
    TracingOutput,
}

impl FocusedPane {
    /// Cycle to the next pane in tab order
    pub fn next(self) -> Self {
        match self {
            Self::TaskHierarchy => Self::TaskConfig,
            Self::TaskConfig => Self::TaskLogs,
            Self::TaskLogs => Self::TracingOutput,
            Self::TracingOutput => Self::TaskHierarchy,
        }
    }

    /// Cycle to the previous pane in tab order
    pub fn previous(self) -> Self {
        match self {
            Self::TaskHierarchy => Self::TracingOutput,
            Self::TaskConfig => Self::TaskHierarchy,
            Self::TaskLogs => Self::TaskConfig,
            Self::TracingOutput => Self::TaskLogs,
        }
    }
}
