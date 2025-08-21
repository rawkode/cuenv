mod bottom_style;
mod focus;
mod tracing;

// Use the new bottom-style implementation as the main TuiApp
pub use self::bottom_style::TuiApp;
pub use self::tracing::tracing_to_tui_event;
