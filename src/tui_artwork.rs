use std::io::BufWriter;
use std::path::{Path, PathBuf};
use tornade_core::utils::AppPaths;

const JPEG_QUALITY: u8 = 70;

/// Compute target pixel dimensions for grid thumbnails from font metrics.
/// `font` = (char_width_px, char_height_px), `img_h` = image height in terminal rows.
pub fn tui_target_size(font: (u16, u16), img_h: u16) -> (u32, u32) {
    let img_cols = if font.0 > 0 {
        ((img_h as u32 * font.1 as u32) / font.0 as u32).max(14) as u16
    } else {
        img_h
    };
    let w = (img_cols as u32 * font.0 as u32).max(1);
    let h = (img_h as u32 * font.1 as u32).max(1);
    (w, h)
}

pub fn tui_album_dir(paths: &AppPaths, target: (u32, u32)) -> PathBuf {
    paths
        .assets_dir()
        .join("tui")
        .join("albums")
        .join(format!("{}x{}", target.0, target.1))
}

pub fn tui_artist_dir(paths: &AppPaths, target: (u32, u32)) -> PathBuf {
    paths
        .assets_dir()
        .join("tui")
        .join("artists")
        .join(format!("{}x{}", target.0, target.1))
}

fn ensure_tui_dirs(paths: &AppPaths, target: (u32, u32)) -> std::io::Result<()> {
    std::fs::create_dir_all(tui_album_dir(paths, target))?;
    std::fs::create_dir_all(tui_artist_dir(paths, target))?;
    Ok(())
}

fn resize_to_jpeg(src: &Path, dst: &Path, target: (u32, u32)) {
    if dst.exists() {
        return;
    }
    let Ok(img) = image::open(src) else { return };
    let resized = img.resize_to_fill(
        target.0,
        target.1,
        image::imageops::FilterType::Triangle,
    );
    let Ok(file) = std::fs::File::create(dst) else { return };
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        BufWriter::new(file),
        JPEG_QUALITY,
    );
    let _ = resized.write_with_encoder(encoder);
}

/// Process all originals in album/artist dirs not yet in tui/ dirs.
/// Meant to run on a background thread.
pub fn process_all_pending(paths: &AppPaths, target: (u32, u32)) {
    if ensure_tui_dirs(paths, target).is_err() {
        return;
    }
    for (src_dir, dst_dir) in [
        (paths.album_artwork_dir(), tui_album_dir(paths, target)),
        (paths.artist_photo_dir(), tui_artist_dir(paths, target)),
    ] {
        let Ok(entries) = std::fs::read_dir(&src_dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let src = entry.path();
            if src.extension().and_then(|e| e.to_str()) != Some("jpg") {
                continue;
            }
            if let Some(filename) = src.file_name() {
                let dst = dst_dir.join(filename);
                resize_to_jpeg(&src, &dst, target);
            }
        }
    }
}

/// Given an original artwork path, return TUI path if it exists, else original path.
pub fn resolve_artwork_path(original: &Path, tui_dir: &Path) -> PathBuf {
    if let Some(filename) = original.file_name() {
        let tui = tui_dir.join(filename);
        if tui.exists() {
            return tui;
        }
    }
    original.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tui_target_size() {
        // font (16, 34), IMG_H = 14
        let (w, h) = tui_target_size((16, 34), 14);
        // img_cols = (14 * 34) / 16 = 29 (>= 14)
        assert_eq!(w, 29 * 16); // 464
        assert_eq!(h, 14 * 34); // 476
    }

    #[test]
    fn test_tui_target_size_zero_font() {
        let (w, h) = tui_target_size((0, 0), 14);
        assert_eq!(w, 1); // .max(1)
        assert_eq!(h, 1);
    }

    #[test]
    fn test_resolve_prefers_tui() {
        let assets_dir =
            std::path::PathBuf::from(env!("HOME")).join(".config/tornade/assets");
        let orig_dir = assets_dir.join("albums");
        // Use a generic tui dir for this test (any existing size subdir)
        let tui_base = assets_dir.join("tui/albums");
        let Ok(size_dirs) = std::fs::read_dir(&tui_base) else {
            return; // skip if no tui dirs exist
        };
        let Some(size_dir) = size_dirs
            .filter_map(|e| e.ok())
            .find(|e| e.path().is_dir())
        else {
            return;
        };
        let tui_dir = size_dir.path();

        let first = std::fs::read_dir(&orig_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jpg"));
        let Some(first) = first else { return };

        let resolved = resolve_artwork_path(&first.path(), &tui_dir);
        if resolved.starts_with(&tui_dir) {
            assert!(resolved.exists());
        }
    }
}
