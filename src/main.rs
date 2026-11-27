//! ACO Musik: the music library and player of the ACO desktop.
//!
//! Reads the music folder (tags and covers with lofty), plays with GStreamer,
//! offers a full window and a mini player, and publishes itself over MPRIS so
//! the control centre and the media keys can steer it.

mod bar;
mod library;
mod mpris;
mod state;
mod ui;

use std::cell::RefCell;
use std::rc::Rc;

use adw::prelude::*;
use gtk::{gio, glib};

use state::State;

const CSS: &str = "
.artwork { border-radius: 10px; background: alpha(currentColor, 0.08); }
.player-bar { padding: 10px 18px; border-top: 1px solid alpha(currentColor, 0.12); background: @headerbar_bg_color; }
.play-button { min-width: 44px; min-height: 44px; background: @accent_bg_color; color: @accent_fg_color; }
.track-list { background: transparent; }
.track-list > row { border-radius: 8px; margin: 0 8px; }
.album-card { padding: 6px; border-radius: 14px; }
.mini-player .play-button { min-width: 40px; min-height: 40px; }
";

/// Add files that were opened from outside (file manager, command line, MPRIS)
/// and play them.
pub fn open_files(state: &Rc<State>, files: &[gio::File]) {
    let mut scan = library::Scan::default();
    for path in files.iter().filter_map(|f| f.path()) {
        if path.is_dir() {
            let inner = library::scan(&path);
            scan.tracks.extend(inner.tracks);
            scan.covers.extend(inner.covers);
        } else if library::is_audio(&path) {
            let track = library::read(&path, &mut scan.covers);
            scan.tracks.push(track);
        }
    }
    let added = state.add(scan);
    if !added.is_empty() {
        state.play_list(added, 0);
    }
}

/// The single player and its window, created on the first start.
type Shared = Rc<RefCell<Option<(Rc<State>, adw::ApplicationWindow)>>>;

fn main() -> glib::ExitCode {
    let shared: Shared = Rc::default();
    aco_common::run("org.aco.Music", true, move |app, files| {
        if shared.borrow().is_none() {
            let state = match State::new() {
                Ok(state) => state,
                Err(e) => {
                    aco_common::fatal(app, "Musik kann nicht spielen", &e);
                    return;
                }
            };
            aco_common::style(CSS);
            let (window, refresh) = ui::build(app, state.clone());
            let raise: Rc<dyn Fn()> = Rc::new({
                let window = window.clone();
                move || window.present()
            });
            mpris::publish(state.clone(), app.clone(), raise);

            aco_common::action(app, &window, "play", &["<Primary>space"], {
                let state = state.clone();
                move || state.toggle()
            });
            aco_common::action(app, &window, "next", &["<Primary>Right"], {
                let state = state.clone();
                move || state.next()
            });
            aco_common::action(app, &window, "previous", &["<Primary>Left"], {
                let state = state.clone();
                move || state.previous()
            });
            aco_common::action(app, &window, "quit", &["<Primary>q"], {
                let app = app.clone();
                move || app.quit()
            });

            // Read the library off the UI thread; the window shows meanwhile.
            let music = aco_common::user_dir(glib::UserDirectory::Music, "Music");
            let (state_, refresh_) = (state.clone(), refresh.clone());
            glib::spawn_future_local(async move {
                if let Ok(scan) = gio::spawn_blocking(move || library::scan(&music)).await {
                    state_.load(scan);
                    refresh_();
                }
            });
            *shared.borrow_mut() = Some((state, window));
        }
        let Some((state, window)) = shared.borrow().clone() else {
            return;
        };
        if !files.is_empty() {
            open_files(&state, files);
        }
        window.present();
    })
}
