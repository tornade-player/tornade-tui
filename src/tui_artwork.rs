use std::io::BufWriter;
use std::path::{Path, PathBuf};
use tornade_core::utils::AppPaths;

const TUI_SIZE: u32 = 128;
const JPEG_QUALITY: u8 = 45;

pub fn tui_album_dir(paths: &AppPaths) -> PathBuf {
    paths.assets_dir().join("tui").join("albums")
}

pub fn tui_artist_dir(paths: &AppPaths) -> PathBuf {
    paths.assets_dir().join("tui").join("artists")
}

fn ensure_tui_dirs(paths: &AppPaths) -> std::io::Result<()> {
    std::fs::create_dir_all(tui_album_dir(paths))?;
    std::fs::create_dir_all(tui_artist_dir(paths))?;
    Ok(())
}

fn resize_to_jpeg(src: &Path, dst: &Path) {
    if dst.exists() {
        return;
    }
    let Ok(img) = image::open(src) else { return };
    let thumb = img.thumbnail(TUI_SIZE, TUI_SIZE);
    let Ok(file) = std::fs::File::create(dst) else { return };
    let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(
        BufWriter::new(file),
        JPEG_QUALITY,
    );
    let _ = thumb.write_with_encoder(encoder);
}

/// Process all originals in album/artist dirs not yet in tui/ dirs.
/// Meant to run on a background thread.
pub fn process_all_pending(paths: &AppPaths) {
    if ensure_tui_dirs(paths).is_err() {
        return;
    }
    for (src_dir, dst_dir) in [
        (paths.album_artwork_dir(), tui_album_dir(paths)),
        (paths.artist_photo_dir(), tui_artist_dir(paths)),
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
                resize_to_jpeg(&src, &dst);
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
    use std::path::PathBuf;

    fn assets_dir() -> PathBuf {
        PathBuf::from(env!("HOME")).join(".config/tornade/assets")
    }

    #[test]
    fn test_tui_thumbnail_exists_and_opens() {
        let tui_dir = assets_dir().join("tui/albums");
        let orig_dir = assets_dir().join("albums");
        let first_orig = std::fs::read_dir(&orig_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jpg"))
            .expect("no original jpg found");
        
        let filename = first_orig.file_name();
        let tui_path = tui_dir.join(&filename);
        assert!(tui_path.exists(), "TUI thumbnail missing: {:?}", tui_path);
        
        let img = image::open(&tui_path).expect("failed to open TUI thumbnail");
        let (w, h) = image::GenericImageView::dimensions(&img);
        assert!(w > 0 && h > 0, "TUI thumbnail has zero dimensions: {}x{}", w, h);
        assert!(w <= 128 && h <= 128, "TUI thumbnail too large: {}x{}", w, h);
        println!("TUI thumbnail: {}x{} color={:?}", w, h, img.color());
    }

    #[test]
    fn test_resolve_prefers_tui() {
        let orig_dir = assets_dir().join("albums");
        let tui_dir = assets_dir().join("tui/albums");
        let first = std::fs::read_dir(&orig_dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .find(|e| e.path().extension().and_then(|x| x.to_str()) == Some("jpg"))
            .unwrap();
        
        let resolved = resolve_artwork_path(&first.path(), &tui_dir);
        assert!(resolved.starts_with(&tui_dir), "Should resolve to TUI dir, got: {:?}", resolved);
        assert!(resolved.exists(), "Resolved path should exist");
    }
}
