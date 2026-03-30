use std::time::Duration;
use tornade_core::{
    models::{RepeatMode, Track},
    services::{PlaybackState, PlayerService},
};

/// Cached snapshot of the player state, polled every 250ms.
pub struct PlayerStateCache {
    pub state: PlaybackState,
    pub current_track: Option<Track>,
    pub position: f64,
    pub volume: f32,
    pub shuffle: bool,
    pub repeat: RepeatMode,
    pub queue: Vec<i64>,
    pub queue_index: usize,
    pub skipped_track_ids: Vec<i64>,
}

impl Default for PlayerStateCache {
    fn default() -> Self {
        Self {
            state: PlaybackState::Stopped,
            current_track: None,
            position: 0.0,
            volume: 1.0,
            shuffle: false,
            repeat: RepeatMode::Off,
            queue: Vec::new(),
            queue_index: 0,
            skipped_track_ids: Vec::new(),
        }
    }
}

impl PlayerStateCache {
    pub fn poll(&mut self, player: &PlayerService) {
        self.state = player.get_state();
        self.current_track = player.get_current_track();
        self.position = player.get_position();
        self.volume = player.get_volume();
        self.shuffle = player.is_shuffle_enabled();
        self.repeat = player.get_repeat_mode();
        self.queue = player.get_queue();
        self.queue_index = player.get_queue_index();
        self.skipped_track_ids = player.get_skipped_track_ids();
    }
}

/// Play from a context: sets the queue to `track_ids` and starts playing at `index`.
pub fn play_from_context(player: &mut PlayerService, track_ids: Vec<i64>, index: usize) {
    let track_id = track_ids.get(index).copied();
    let _ = player.set_queue(track_ids);
    if let Some(id) = track_id {
        let _ = player.play(id);
    }
}

/// Format a mm:ss string into a Duration, returns None on parse failure.
pub fn parse_seek_position(s: &str) -> Option<Duration> {
    let parts: Vec<&str> = s.split(':').collect();
    match parts.as_slice() {
        [mm, ss] => {
            let mins: u64 = mm.parse().ok()?;
            let secs: u64 = ss.parse().ok()?;
            Some(Duration::from_secs(mins * 60 + secs))
        }
        [ss] => {
            let secs: u64 = ss.parse().ok()?;
            Some(Duration::from_secs(secs))
        }
        _ => None,
    }
}
