//! Cross-platform hardware media-key integration (feature 012, User Story 4).
//!
//! Uses the `souvlaki` crate to respond to the play/pause, next, and previous
//! media keys on macOS, Windows, and Linux, and to publish now-playing
//! information to the operating system.
//!
//! Per-platform mechanism:
//! - **Linux**: MPRIS over D-Bus (`org.mpris.MediaPlayer2`). Requires a D-Bus
//!   session bus; on a bare TTY without one, [`init`] returns `None`.
//! - **Windows**: `SystemMediaTransportControls` (SMTC). `souvlaki` supplies a
//!   hidden window, so `hwnd: None` is accepted.
//! - **macOS**: `MPNowPlayingInfoCenter` + `MPRemoteCommandCenter`. CAVEAT:
//!   these deliver remote-command callbacks via the main run loop. A pure
//!   terminal process does not run an `NSApplication` run loop, so on macOS the
//!   callbacks may not fire in a plain TTY. Now-playing metadata still updates.
//!   This is why [`init`] degrades gracefully and the whole feature is optional
//!   (FR-026): the player runs normally with all keyboard controls whether or
//!   not media keys are delivered.
//!
//! Initialization NEVER blocks the UI thread and NEVER panics: any failure
//! yields `None` and a warning log.

use std::sync::mpsc::Sender;
use std::time::{Duration, Instant};

use souvlaki::{
    MediaControlEvent, MediaControls, MediaMetadata, MediaPlayback, MediaPosition, PlatformConfig,
};

/// A normalized media-key event delivered from the OS callback thread onto the
/// application channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyEvent {
    Toggle,
    Play,
    Pause,
    Next,
    Previous,
}

/// Default debounce window for repeated toggle events (FR-027).
pub const TOGGLE_DEBOUNCE: Duration = Duration::from_millis(250);

/// Map a raw `souvlaki` event to our transport-only [`MediaKeyEvent`].
///
/// Returns `None` for events this player does not act on (seek, volume, etc.).
pub fn normalize(event: &MediaControlEvent) -> Option<MediaKeyEvent> {
    match event {
        MediaControlEvent::Toggle => Some(MediaKeyEvent::Toggle),
        MediaControlEvent::Play => Some(MediaKeyEvent::Play),
        MediaControlEvent::Pause => Some(MediaKeyEvent::Pause),
        MediaControlEvent::Next => Some(MediaKeyEvent::Next),
        MediaControlEvent::Previous => Some(MediaKeyEvent::Previous),
        _ => None,
    }
}

/// Coalesces rapidly repeated toggle events so a single physical press cannot
/// produce a double play/pause (FR-027). Pure and time-injectable for testing.
pub struct Debouncer {
    window: Duration,
    last: Option<Instant>,
}

impl Debouncer {
    pub fn new(window: Duration) -> Self {
        Self { window, last: None }
    }

    /// Returns `true` if an event at `now` should be accepted, `false` if it
    /// falls within the debounce window of the previously accepted event.
    pub fn accept(&mut self, now: Instant) -> bool {
        match self.last {
            Some(prev) if now.duration_since(prev) < self.window => false,
            _ => {
                self.last = Some(now);
                true
            }
        }
    }
}

/// Live handle kept alive by the app to publish now-playing state to the OS.
/// Dropping it detaches the OS media controls.
pub struct MediaKeyHandle {
    controls: MediaControls,
}

impl MediaKeyHandle {
    /// Publish the currently playing item's metadata to the OS now-playing UI.
    pub fn update_now_playing(
        &mut self,
        title: &str,
        artist: Option<&str>,
        album: Option<&str>,
        duration: Option<Duration>,
    ) {
        let _ = self.controls.set_metadata(MediaMetadata {
            title: Some(title),
            artist,
            album,
            cover_url: None,
            duration,
        });
    }

    /// Reflect the play/pause state and playback position to the OS.
    pub fn set_playback(&mut self, playing: bool, progress: Duration) {
        let pos = Some(MediaPosition(progress));
        let playback = if playing {
            MediaPlayback::Playing { progress: pos }
        } else {
            MediaPlayback::Paused { progress: pos }
        };
        let _ = self.controls.set_playback(playback);
    }

    /// Reflect a fully stopped state to the OS.
    pub fn set_stopped(&mut self) {
        let _ = self.controls.set_playback(MediaPlayback::Stopped);
    }
}

/// Initialize OS media controls and start delivering media-key events over
/// `sender`. Returns `None` (with a warning) if the platform denies or does not
/// support application media controls, in which case the caller continues with
/// all existing keyboard controls intact (FR-026).
pub fn init(sender: Sender<MediaKeyEvent>) -> Option<MediaKeyHandle> {
    let config = PlatformConfig {
        display_name: "Tornade",
        dbus_name: "tornade",
        hwnd: None,
    };

    let mut controls = match MediaControls::new(config) {
        Ok(c) => c,
        Err(e) => {
            log::warn!("media keys unavailable (controls init failed): {e:?}");
            return None;
        }
    };

    let attach = controls.attach(move |event: MediaControlEvent| {
        if let Some(ev) = normalize(&event) {
            // The UI thread may have exited; ignore send errors.
            let _ = sender.send(ev);
        }
    });

    if let Err(e) = attach {
        log::warn!("media keys unavailable (attach failed): {e:?}");
        return None;
    }

    let mut handle = MediaKeyHandle { controls };
    handle.set_stopped();
    Some(handle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_maps_transport_events() {
        assert_eq!(
            normalize(&MediaControlEvent::Toggle),
            Some(MediaKeyEvent::Toggle)
        );
        assert_eq!(
            normalize(&MediaControlEvent::Play),
            Some(MediaKeyEvent::Play)
        );
        assert_eq!(
            normalize(&MediaControlEvent::Pause),
            Some(MediaKeyEvent::Pause)
        );
        assert_eq!(
            normalize(&MediaControlEvent::Next),
            Some(MediaKeyEvent::Next)
        );
        assert_eq!(
            normalize(&MediaControlEvent::Previous),
            Some(MediaKeyEvent::Previous)
        );
    }

    #[test]
    fn normalize_ignores_non_transport_events() {
        assert_eq!(normalize(&MediaControlEvent::Stop), None);
        assert_eq!(normalize(&MediaControlEvent::Raise), None);
        assert_eq!(normalize(&MediaControlEvent::SetVolume(0.5)), None);
    }

    #[test]
    fn debouncer_rejects_repeats_within_window() {
        let mut d = Debouncer::new(Duration::from_millis(250));
        let t0 = Instant::now();
        assert!(d.accept(t0), "first event accepted");
        assert!(
            !d.accept(t0 + Duration::from_millis(100)),
            "repeat within window rejected"
        );
        assert!(
            !d.accept(t0 + Duration::from_millis(249)),
            "repeat just inside window rejected"
        );
        assert!(
            d.accept(t0 + Duration::from_millis(300)),
            "event past window accepted"
        );
        assert!(
            !d.accept(t0 + Duration::from_millis(400)),
            "repeat within window of the last accepted rejected"
        );
    }
}
