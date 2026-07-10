//! Cross-platform hardware media-key integration (feature 012, User Story 4).
//!
//! Uses the `souvlaki` crate to respond to the play/pause, next, and previous
//! media keys on macOS, Windows, and Linux, and to publish now-playing
//! information to the operating system. Initialization degrades gracefully:
//! if the platform denies media-control access, the player continues to run
//! with all existing keyboard controls intact (FR-026).
//!
//! Implemented in later tasks (T055-T060).

/// A normalized media-key event delivered from the OS callback thread onto the
/// application event channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyEvent {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
}
