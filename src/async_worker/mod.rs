//! Worker thread bridging the synchronous TUI event loop to the async core
//! MusicBrainz / artwork calls (feature 012, User Story 3).
//!
//! Online actions (`scrape_*`, artwork fetch) are `async` in `tornade-core`.
//! The TUI loop is synchronous (`event::poll`), so these run on a dedicated
//! worker thread backed by a current-thread `tokio` runtime, with results
//! delivered back over an `mpsc` channel and bounded by a ~10s timeout so the
//! UI never hangs (FR-022, SC-005).
//!
//! Implemented in later tasks (T047-T049).
