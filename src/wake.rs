//! Global wake signal for the event-driven main loop.
//!
//! The run loop blocks on a single channel instead of polling. Background
//! workers (image decode threads, the async worker, media keys) call
//! [`signal`] to wake the loop the instant they have something to show, so the
//! UI redraws immediately without a busy poll and stays idle otherwise.

use std::sync::OnceLock;
use std::sync::mpsc::Sender;

/// A loop wake-up. `Input` carries a terminal event from the reader thread;
/// `Signal` is a bare "something changed, please redraw" nudge.
pub enum Wake {
    Input(ratatui::crossterm::event::Event),
    Signal,
}

static WAKE_TX: OnceLock<Sender<Wake>> = OnceLock::new();

/// Register the loop's wake sender. Called once at startup.
pub fn init(tx: Sender<Wake>) {
    let _ = WAKE_TX.set(tx);
}

/// Nudge the run loop to redraw. No-op before [`init`] or after the loop exits.
pub fn signal() {
    if let Some(tx) = WAKE_TX.get() {
        let _ = tx.send(Wake::Signal);
    }
}
