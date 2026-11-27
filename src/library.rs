//! The music library: every audio file below the music folder with its tags.
//! Tags and cover art are read with lofty (pure Rust, no extra system library).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lofty::prelude::*;

const EXTENSIONS: &[&str] = &[
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "aiff", "ape", "mpc", "wv",
];

#[derive(Clone, Debug)]
pub struct Track {
    pub path: PathBuf,
    pub title: String,
    pub artist: String,
    pub album: String,
    pub number: u32,
    pub duration: f64,
}

impl Track {
    /// Albums are told apart by album and artist.
    pub fn album_key(&self) -> String {
        format!("{}\u{1f}{}", self.album, self.artist)
    }
}

/// Result of a scan: tracks plus one cover per album (raw image bytes).
#[derive(Default)]
pub struct Scan {
    pub tracks: Vec<Track>,
    pub covers: HashMap<String, Arc<[u8]>>,
}

pub fn is_audio(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| EXTENSIONS.contains(&e.to_ascii_lowercase().as_str()))
}

/// Read one file; untagged files get their file name as title.
pub fn read(path: &Path, covers: &mut HashMap<String, Arc<[u8]>>) -> Track {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut track = Track {
        path: path.to_owned(),
        title: stem,
        artist: "Unbekannter Künstler".into(),
        album: "Unbekanntes Album".into(),
        number: 0,
        duration: 0.0,
    };
    if let Ok(tagged) = lofty::read_from_path(path) {
        track.duration = tagged.properties().duration().as_secs_f64();
        if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
            if let Some(v) = tag.title() {
                track.title = v.into_owned();
            }
            if let Some(v) = tag.artist() {
                track.artist = v.into_owned();
            }
            if let Some(v) = tag.album() {
                track.album = v.into_owned();
            }
            track.number = tag.track().unwrap_or(0);
            if let Some(picture) = tag.pictures().first() {
                covers
                    .entry(track.album_key())
                    .or_insert_with(|| Arc::from(picture.data()));
            }
        }
    }
    // A cover.jpg / folder.png next to the files counts as well.
    let key = track.album_key();
    if !covers.contains_key(&key) {
        if let Some(dir) = path.parent() {
            for name in [
                "cover.jpg",
                "cover.png",
                "folder.jpg",
                "folder.png",
                "Cover.jpg",
                "Folder.jpg",
            ] {
                if let Ok(bytes) = std::fs::read(dir.join(name)) {
                    covers.insert(key, Arc::from(bytes));
                    break;
                }
            }
        }
    }
    track
}

/// Every audio file below `root` (hidden folders skipped), sorted by artist,
/// album, track number and title.
pub fn scan(root: &Path) -> Scan {
    let mut scan = Scan::default();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if entry.file_name().to_string_lossy().starts_with('.') {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else if is_audio(&path) {
                let track = read(&path, &mut scan.covers);
                scan.tracks.push(track);
            }
        }
    }
    scan.tracks.sort_by(|a, b| {
        (
            a.artist.to_lowercase(),
            a.album.to_lowercase(),
            a.number,
            a.title.to_lowercase(),
        )
            .cmp(&(
                b.artist.to_lowercase(),
                b.album.to_lowercase(),
                b.number,
                b.title.to_lowercase(),
            ))
    });
    scan
}
