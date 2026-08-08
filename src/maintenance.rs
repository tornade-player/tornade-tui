//! Library maintenance — reconciling the database against the filesystem.
//!
//! `:cleanup` used to check only whether source *directories* were still
//! reachable. A track whose file had been deleted, moved, or converted to a
//! different format stayed in the library indefinitely, still advertising the
//! format it was first scanned as. This removes those rows.

use std::path::PathBuf;

use rusqlite::params;
use tornade_core::db::DbPool;

/// What a cleanup pass removed.
#[derive(Debug, Default, Clone, Copy)]
pub struct CleanupReport {
    pub tracks_removed: usize,
    pub albums_pruned: usize,
    pub artists_pruned: usize,
    pub genres_pruned: usize,
    /// Sources whose root directory is currently unreachable. Their tracks are
    /// deliberately left untouched — see [`clean_missing_tracks`].
    pub skipped_sources: usize,
}

impl CleanupReport {
    /// True when nothing needed removing.
    pub fn is_clean(&self) -> bool {
        self.tracks_removed == 0
            && self.albums_pruned == 0
            && self.artists_pruned == 0
            && self.genres_pruned == 0
    }

    /// One-line summary for the status bar.
    pub fn summary(&self) -> String {
        if self.is_clean() {
            let mut msg = String::from("Library is clean — no missing files");
            if self.skipped_sources > 0 {
                msg.push_str(&format!(
                    " ({} source(s) offline, skipped)",
                    self.skipped_sources
                ));
            }
            return msg;
        }

        let mut parts = vec![format!("Removed {} missing track(s)", self.tracks_removed)];
        if self.albums_pruned > 0 {
            parts.push(format!("{} album(s)", self.albums_pruned));
        }
        if self.artists_pruned > 0 {
            parts.push(format!("{} artist(s)", self.artists_pruned));
        }
        if self.genres_pruned > 0 {
            parts.push(format!("{} genre(s)", self.genres_pruned));
        }
        let mut msg = parts.join(", ");
        if self.skipped_sources > 0 {
            msg.push_str(&format!(
                " — {} source(s) offline and skipped",
                self.skipped_sources
            ));
        }
        msg
    }
}

/// Drop every track whose backing file has gone, then prune the albums,
/// artists and genres left with nothing pointing at them.
///
/// Sources whose root directory is unreachable are skipped entirely. An
/// unmounted NAS or an unplugged drive makes every one of its files look
/// missing, and deleting on that basis would wipe the library — along with its
/// ratings and playlist entries — the moment a disk went offline. Only files
/// missing from a source that is demonstrably present are removed.
pub fn clean_missing_tracks(pool: &DbPool) -> Result<CleanupReport, String> {
    let mut report = CleanupReport::default();
    let mut missing: Vec<i64> = Vec::new();

    {
        let conn = pool.get().map_err(|e| e.to_string())?;

        let sources: Vec<(i64, Option<String>)> = conn
            .prepare("SELECT id, path FROM sources")
            .and_then(|mut stmt| {
                stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
                    .and_then(|rows| rows.collect())
            })
            .map_err(|e| e.to_string())?;

        let mut stmt = conn
            .prepare("SELECT id, file_path FROM tracks WHERE source_id = ?1")
            .map_err(|e| e.to_string())?;

        for (source_id, root) in sources {
            if let Some(root) = root
                && !PathBuf::from(&root).exists()
            {
                report.skipped_sources += 1;
                continue;
            }

            let rows: Vec<(i64, String)> = stmt
                .query_map(params![source_id], |row| Ok((row.get(0)?, row.get(1)?)))
                .and_then(|rows| rows.collect())
                .map_err(|e| e.to_string())?;

            missing.extend(
                rows.into_iter()
                    .filter(|(_, path)| !PathBuf::from(path).exists())
                    .map(|(id, _)| id),
            );
        }
    }

    if missing.is_empty() {
        return Ok(report);
    }

    let mut conn = pool.get().map_err(|e| e.to_string())?;
    let tx = conn.transaction().map_err(|e| e.to_string())?;

    // These child tables declare ON DELETE CASCADE, but SQLite only honours it
    // when `PRAGMA foreign_keys` is on and nothing in tornade-core turns it on.
    // Delete the children explicitly rather than change the pragma on a pooled
    // connection that the rest of the app goes on to reuse.
    for id in &missing {
        for sql in [
            "DELETE FROM playlist_tracks WHERE track_id = ?1",
            "DELETE FROM track_genres WHERE track_id = ?1",
            "DELETE FROM track_artists WHERE track_id = ?1",
            // The tracks_ad trigger keeps tracks_fts in step with this one.
            "DELETE FROM tracks WHERE id = ?1",
        ] {
            tx.execute(sql, params![id]).map_err(|e| e.to_string())?;
        }
    }
    report.tracks_removed = missing.len();

    report.albums_pruned = tx
        .execute(
            "DELETE FROM albums
              WHERE id NOT IN (SELECT album_id FROM tracks WHERE album_id IS NOT NULL)",
            [],
        )
        .map_err(|e| e.to_string())?;

    // albums.artist_id is NOT NULL, so an artist is still in use if any album
    // references it, even once its tracks are gone.
    report.artists_pruned = tx
        .execute(
            "DELETE FROM artists
              WHERE id NOT IN (SELECT artist_id FROM tracks)
                AND id NOT IN (SELECT artist_id FROM track_artists)
                AND id NOT IN (SELECT artist_id FROM albums)",
            [],
        )
        .map_err(|e| e.to_string())?;

    report.genres_pruned = tx
        .execute(
            "DELETE FROM genres WHERE id NOT IN (SELECT genre_id FROM track_genres)",
            [],
        )
        .map_err(|e| e.to_string())?;

    tx.commit().map_err(|e| e.to_string())?;

    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::Path;
    use tornade_core::db::{create_pool, initialize_database};

    /// An isolated database with a music directory beside it.
    fn fixture(name: &str) -> (DbPool, PathBuf) {
        let dir = std::env::temp_dir().join(format!("tornade-maint-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("music")).unwrap();

        let pool = create_pool(dir.join("library.db")).unwrap();
        initialize_database(&pool).unwrap();
        (pool, dir)
    }

    /// Register one source with one album by one artist. Each entry is
    /// (filename, whether the file actually exists on disk).
    fn seed(pool: &DbPool, root: &Path, tracks: &[(&str, bool)]) {
        let conn = pool.get().unwrap();
        conn.execute(
            "INSERT INTO sources (name, type, path) VALUES ('test', 'disk', ?1)",
            params![root.to_str().unwrap()],
        )
        .unwrap();
        let source_id = conn.last_insert_rowid();

        conn.execute("INSERT INTO artists (name) VALUES ('Test Artist')", [])
            .unwrap();
        let artist_id = conn.last_insert_rowid();

        conn.execute(
            "INSERT INTO albums (title, artist_id) VALUES ('Test Album', ?1)",
            params![artist_id],
        )
        .unwrap();
        let album_id = conn.last_insert_rowid();

        for (name, present) in tracks {
            let path = root.join(name);
            if *present {
                fs::write(&path, b"audio").unwrap();
            }
            conn.execute(
                "INSERT INTO tracks
                   (title, album_id, artist_id, source_id, file_path,
                    duration, file_type, file_size)
                 VALUES (?1, ?2, ?3, ?4, ?5, 1, 'flac', 5)",
                params![name, album_id, artist_id, source_id, path.to_str().unwrap()],
            )
            .unwrap();
        }
    }

    fn count(pool: &DbPool, table: &str) -> i64 {
        pool.get()
            .unwrap()
            .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
            .unwrap()
    }

    #[test]
    fn removes_only_tracks_whose_file_is_gone() {
        let (pool, dir) = fixture("missing");
        let music = dir.join("music");
        seed(
            &pool,
            &music,
            &[("a.flac", true), ("b.flac", true), ("gone.flac", false)],
        );

        let report = clean_missing_tracks(&pool).unwrap();

        assert_eq!(report.tracks_removed, 1);
        assert_eq!(report.skipped_sources, 0);
        assert_eq!(count(&pool, "tracks"), 2);
        // The tracks_ad trigger must keep the search index in step, or search
        // keeps returning rows that no longer exist.
        assert_eq!(count(&pool, "tracks_fts"), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn offline_source_is_skipped_not_emptied() {
        let (pool, dir) = fixture("offline");
        let music = dir.join("music");
        seed(&pool, &music, &[("a.flac", true), ("b.flac", true)]);

        // The drive goes away: every file under it now looks missing.
        fs::remove_dir_all(&music).unwrap();

        let report = clean_missing_tracks(&pool).unwrap();

        assert_eq!(
            report.tracks_removed, 0,
            "an unmounted source must never wipe its tracks"
        );
        assert_eq!(report.skipped_sources, 1);
        assert_eq!(count(&pool, "tracks"), 2);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn prunes_album_and_artist_left_with_nothing() {
        let (pool, dir) = fixture("orphans");
        let music = dir.join("music");
        seed(&pool, &music, &[("gone.flac", false)]);

        let report = clean_missing_tracks(&pool).unwrap();

        assert_eq!(report.tracks_removed, 1);
        assert_eq!(report.albums_pruned, 1);
        assert_eq!(report.artists_pruned, 1);
        assert_eq!(count(&pool, "albums"), 0);
        assert_eq!(count(&pool, "artists"), 0);

        fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn clean_library_reports_nothing_to_do() {
        let (pool, dir) = fixture("clean");
        let music = dir.join("music");
        seed(&pool, &music, &[("a.flac", true)]);

        let report = clean_missing_tracks(&pool).unwrap();

        assert!(report.is_clean());
        assert_eq!(count(&pool, "tracks"), 1);

        fs::remove_dir_all(&dir).ok();
    }
}
