//! Playback state shared by the window, the mini player and MPRIS: the
//! library, the queue, shuffle and repeat, and the GStreamer player.

use std::cell::{Cell, RefCell};
use std::collections::{hash_map::Entry, HashMap};
use std::rc::Rc;

use aco_common::player::{Event, Player};
use gtk::{gdk, glib};

use crate::library::{Scan, Track};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Repeat {
    Off,
    All,
    One,
}

pub struct State {
    pub player: Rc<Player>,
    pub tracks: RefCell<Vec<Track>>,
    pub covers: RefCell<HashMap<String, gdk::Texture>>,
    /// Raw cover bytes, kept for MPRIS (written to the cache when a track plays).
    pub cover_bytes: RefCell<HashMap<String, std::sync::Arc<[u8]>>>,
    /// Indices into `tracks`, in playing order.
    pub queue: RefCell<Vec<usize>>,
    pub position: Cell<Option<usize>>,
    pub shuffle: Cell<bool>,
    pub repeat: Cell<Repeat>,
    listeners: RefCell<Vec<Box<dyn Fn()>>>,
}

impl State {
    pub fn new() -> Result<Rc<Self>, String> {
        let state: Rc<RefCell<Option<std::rc::Weak<State>>>> = Rc::default();
        let weak = state.clone();
        let player = Player::new(false, move |event| {
            let Some(state) = weak.borrow().as_ref().and_then(|w| w.upgrade()) else {
                return;
            };
            match event {
                Event::Finished => state.finished(),
                Event::State(_) | Event::Duration(_) => state.changed(),
                Event::Error(e) => {
                    eprintln!("aco-music: {e}");
                    state.next();
                }
                Event::Streams => {}
            }
        })?;
        let this = Rc::new(Self {
            player,
            tracks: RefCell::default(),
            covers: RefCell::default(),
            cover_bytes: RefCell::default(),
            queue: RefCell::default(),
            position: Cell::new(None),
            shuffle: Cell::new(false),
            repeat: Cell::new(Repeat::Off),
            listeners: RefCell::default(),
        });
        *state.borrow_mut() = Some(Rc::downgrade(&this));
        Ok(this)
    }

    /// Merge scanned tracks without changing any existing playback queue index.
    pub fn add(&self, scan: Scan) -> Vec<usize> {
        for (key, bytes) in scan.covers {
            if let Ok(texture) = gdk::Texture::from_bytes(&glib::Bytes::from(&bytes[..])) {
                self.covers.borrow_mut().insert(key.clone(), texture);
            }
            self.cover_bytes.borrow_mut().insert(key, bytes);
        }
        merge_tracks(&mut self.tracks.borrow_mut(), scan.tracks)
    }

    /// Call `f` whenever the current track or the playing state changes.
    pub fn subscribe(&self, f: impl Fn() + 'static) {
        self.listeners.borrow_mut().push(Box::new(f));
    }

    pub fn changed(&self) {
        for listener in self.listeners.borrow().iter() {
            listener();
        }
    }

    pub fn current(&self) -> Option<Track> {
        let index = *self.queue.borrow().get(self.position.get()?)?;
        self.tracks.borrow().get(index).cloned()
    }

    pub fn cover(&self, track: &Track) -> Option<gdk::Texture> {
        self.covers.borrow().get(&track.album_key()).cloned()
    }

    /// Play `tracks` (library indices) starting with `start`.
    pub fn play_list(&self, tracks: Vec<usize>, start: usize) {
        if tracks.is_empty() {
            return;
        }
        let mut queue = tracks;
        let mut first = start.min(queue.len().saturating_sub(1));
        if self.shuffle.get() {
            let chosen = queue.remove(first);
            shuffle(&mut queue);
            queue.insert(0, chosen);
            first = 0;
        }
        *self.queue.borrow_mut() = queue;
        self.play_at(first);
    }

    pub fn play_at(&self, position: usize) {
        let Some(&index) = self.queue.borrow().get(position) else {
            return;
        };
        let Some(path) = self.tracks.borrow().get(index).map(|t| t.path.clone()) else {
            return;
        };
        self.position.set(Some(position));
        self.player.open(&path, None);
        self.changed();
    }

    pub fn toggle(&self) {
        if self.position.get().is_none() {
            let all: Vec<usize> = (0..self.tracks.borrow().len()).collect();
            if !all.is_empty() {
                self.play_list(all, 0);
            }
            return;
        }
        self.player.toggle();
        self.changed();
    }

    pub fn next(&self) {
        let Some(position) = self.position.get() else {
            return;
        };
        let len = self.queue.borrow().len();
        if position + 1 < len {
            self.play_at(position + 1);
        } else if self.repeat.get() == Repeat::All && len > 0 {
            self.play_at(0);
        } else {
            self.player.stop();
            self.changed();
        }
    }

    /// Back to the start of the track after 3 seconds, else the previous one.
    pub fn previous(&self) {
        let Some(position) = self.position.get() else {
            return;
        };
        if self.player.position().unwrap_or(0.0) > 3.0 || position == 0 {
            self.player.seek(0.0);
        } else {
            self.play_at(position - 1);
        }
    }

    fn finished(&self) {
        if self.repeat.get() == Repeat::One {
            if let Some(p) = self.position.get() {
                self.play_at(p);
            }
        } else {
            self.next();
        }
    }

    pub fn set_shuffle(&self, on: bool) {
        self.shuffle.set(on);
        // Shuffle what is still to come, keep the current track.
        if on {
            if let Some(p) = self.position.get() {
                let mut queue = self.queue.borrow_mut();
                if p + 1 < queue.len() {
                    shuffle(&mut queue[p + 1..]);
                }
            }
        }
        self.changed();
    }

    pub fn cycle_repeat(&self) {
        self.repeat.set(match self.repeat.get() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        });
        self.changed();
    }
}

fn merge_tracks(tracks: &mut Vec<Track>, incoming: Vec<Track>) -> Vec<usize> {
    let mut existing: HashMap<_, _> = tracks
        .iter()
        .enumerate()
        .map(|(index, track)| (track.path.clone(), index))
        .collect();
    incoming
        .into_iter()
        .map(|track| match existing.entry(track.path.clone()) {
            Entry::Occupied(entry) => {
                let index = *entry.get();
                tracks[index] = track;
                index
            }
            Entry::Vacant(entry) => {
                let index = tracks.len();
                entry.insert(index);
                tracks.push(track);
                index
            }
        })
        .collect()
}

/// Fisher–Yates with GLib's random numbers.
fn shuffle<T>(items: &mut [T]) {
    for i in (1..items.len()).rev() {
        let j = glib::random_int_range(0, i as i32 + 1) as usize;
        items.swap(i, j);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_preserves_indices_for_tracks_opened_during_startup() {
        let track = |path: &str, title: &str| Track {
            path: path.into(),
            title: title.into(),
            artist: String::new(),
            album: String::new(),
            number: 0,
            duration: 0.0,
        };
        let mut tracks = vec![
            track("second.flac", "From file"),
            track("outside.flac", "External"),
        ];
        let indices = merge_tracks(
            &mut tracks,
            vec![track("first.flac", "First"), track("second.flac", "Second")],
        );

        assert_eq!(indices, [2, 0]);
        assert_eq!(tracks.len(), 3);
        assert_eq!(tracks[0].title, "Second");
        assert_eq!(tracks[1].title, "External");
    }
}
