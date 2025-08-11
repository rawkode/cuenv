#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPane {
    MiniMap,
    TaskDetails,
    Environment,
}

impl FocusedPane {
    /// Cycle to the next pane in tab order
    pub fn next(self) -> Self {
        match self {
            Self::MiniMap => Self::TaskDetails,
            Self::TaskDetails => Self::Environment,
            Self::Environment => Self::MiniMap,
        }
    }
}
