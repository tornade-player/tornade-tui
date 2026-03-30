use std::path::PathBuf;
use tornade_core::models::AudioFormat;

pub fn expand_tilde(s: &str) -> PathBuf {
    if s.starts_with("~/") || s == "~" {
        if let Ok(home) = std::env::var("HOME") {
            return PathBuf::from(format!("{}{}", home, &s[1..]));
        }
    }
    PathBuf::from(s)
}

pub fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        format!("{}…", s.chars().take(max.saturating_sub(1)).collect::<String>())
    }
}

pub fn format_duration(secs: u64) -> String {
    format!("{}:{:02}", secs / 60, secs % 60)
}

pub fn format_audio(fmt: AudioFormat, sample_rate: Option<u32>, bit_depth: Option<u8>) -> String {
    let f = match fmt {
        AudioFormat::Flac => "FLAC",
        AudioFormat::Mp3 => "MP3",
        AudioFormat::Aac => "AAC",
        AudioFormat::Alac => "ALAC",
    };
    match (sample_rate, bit_depth) {
        (Some(sr), Some(bd)) => format!("{} {}bit {}kHz", f, bd, sr / 1000),
        (Some(sr), None) => format!("{} {}kHz", f, sr / 1000),
        _ => f.to_string(),
    }
}

pub fn format_rating(r: u8) -> String {
    let filled = r as usize;
    let empty = 5usize.saturating_sub(filled);
    format!("{}{}", "★".repeat(filled), "☆".repeat(empty))
}
