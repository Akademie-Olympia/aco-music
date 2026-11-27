//! The player bar under the library and the mini player: a small card with
//! artwork, title, progress and controls that can replace the window.

use std::rc::Rc;

use adw::prelude::*;
use gtk::glib;

use crate::state::State;
use crate::ui::{artwork, follow_progress, repeat_icon, set_art};

fn flat(icon: &str, tooltip: &str) -> gtk::Button {
    let button = aco_common::icon_button(icon, tooltip);
    button.add_css_class("flat");
    button.add_css_class("circular");
    button.set_valign(gtk::Align::Center);
    button
}

fn play_button() -> gtk::Button {
    let button = aco_common::icon_button("media-playback-start-symbolic", "Abspielen");
    button.add_css_class("circular");
    button.add_css_class("play-button");
    button.set_valign(gtk::Align::Center);
    button
}

/// Keep title, artist, artwork and the play button in step with the player.
fn show_now_playing(
    state: &Rc<State>,
    title: &gtk::Label,
    artist: &gtk::Label,
    art: &gtk::Image,
    play: &gtk::Button,
) {
    let state_ = state.clone();
    let (title, artist, art, play) = (title.clone(), artist.clone(), art.clone(), play.clone());
    let update = move || {
        let track = state_.current();
        title.set_label(
            track
                .as_ref()
                .map_or("Nichts wird gespielt", |t| t.title.as_str()),
        );
        artist.set_label(track.as_ref().map_or("", |t| t.artist.as_str()));
        set_art(&art, &state_, track.as_ref());
        let playing = state_.player.is_playing();
        play.set_icon_name(if playing {
            "media-playback-pause-symbolic"
        } else {
            "media-playback-start-symbolic"
        });
        play.set_tooltip_text(Some(if playing { "Pause" } else { "Abspielen" }));
    };
    update();
    state.subscribe(update);
}

fn connect_controls(
    state: &Rc<State>,
    previous: &gtk::Button,
    play: &gtk::Button,
    next: &gtk::Button,
) {
    previous.connect_clicked({
        let state = state.clone();
        move |_| state.previous()
    });
    play.connect_clicked({
        let state = state.clone();
        move |_| state.toggle()
    });
    next.connect_clicked({
        let state = state.clone();
        move |_| state.next()
    });
}

fn progress(state: &Rc<State>) -> (gtk::Scale, gtk::Label, gtk::Label) {
    let scale = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 1.0);
    scale.set_hexpand(true);
    scale.add_css_class("progress");
    let elapsed = gtk::Label::builder()
        .label("0:00")
        .css_classes(["caption", "numeric", "dim-label"])
        .build();
    let remaining = gtk::Label::builder()
        .label("−0:00")
        .css_classes(["caption", "numeric", "dim-label"])
        .build();
    follow_progress(
        state.clone(),
        scale.clone(),
        elapsed.clone(),
        remaining.clone(),
    );
    (scale, elapsed, remaining)
}

pub fn player_bar(state: Rc<State>, app: &adw::Application) -> gtk::Box {
    // Left: what is playing.
    let art = artwork(56);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["heading"])
        .build();
    let artist = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label", "caption"])
        .build();
    let names = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .valign(gtk::Align::Center)
        .build();
    names.append(&title);
    names.append(&artist);
    let left = gtk::Box::builder().spacing(12).width_request(280).build();
    left.append(&art);
    left.append(&names);

    // Centre: controls over the progress.
    let shuffle = gtk::ToggleButton::builder()
        .icon_name("media-playlist-shuffle-symbolic")
        .tooltip_text("Zufällige Reihenfolge")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .build();
    let previous = flat("media-skip-backward-symbolic", "Zurück");
    let play = play_button();
    let next = flat("media-skip-forward-symbolic", "Weiter");
    let repeat = gtk::ToggleButton::builder()
        .icon_name("media-playlist-repeat-symbolic")
        .css_classes(["flat", "circular"])
        .valign(gtk::Align::Center)
        .build();
    let controls = gtk::Box::builder()
        .spacing(10)
        .halign(gtk::Align::Center)
        .build();
    for w in [
        shuffle.upcast_ref::<gtk::Widget>(),
        previous.upcast_ref(),
        play.upcast_ref(),
        next.upcast_ref(),
        repeat.upcast_ref(),
    ] {
        controls.append(w);
    }
    let (scale, elapsed, remaining) = progress(&state);
    let line = gtk::Box::builder().spacing(10).build();
    line.append(&elapsed);
    line.append(&scale);
    line.append(&remaining);
    let centre = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .hexpand(true)
        .width_request(320)
        .build();
    centre.append(&controls);
    centre.append(&line);

    // Right: volume and the mini player.
    let mute = flat("audio-volume-high-symbolic", "Stumm");
    let volume = gtk::Scale::with_range(gtk::Orientation::Horizontal, 0.0, 1.0, 0.01);
    volume.set_value(state.player.volume());
    volume.set_width_request(110);
    let mini = flat("view-restore-symbolic", "Mini-Player");
    let right = gtk::Box::builder()
        .spacing(6)
        .width_request(280)
        .halign(gtk::Align::End)
        .build();
    right.append(&mute);
    right.append(&volume);
    right.append(&mini);

    let bar = gtk::Box::builder()
        .spacing(24)
        .css_classes(["player-bar"])
        .build();
    bar.append(&left);
    bar.append(&centre);
    bar.append(&right);

    show_now_playing(&state, &title, &artist, &art, &play);
    connect_controls(&state, &previous, &play, &next);
    shuffle.connect_toggled({
        let state = state.clone();
        move |b| {
            if b.is_active() != state.shuffle.get() {
                state.set_shuffle(b.is_active());
            }
        }
    });
    repeat.connect_clicked({
        let state = state.clone();
        move |_| state.cycle_repeat()
    });
    state.subscribe({
        let (state, shuffle, repeat) = (state.clone(), shuffle.clone(), repeat.clone());
        move || {
            shuffle.set_active(state.shuffle.get());
            let (icon, tip, on) = repeat_icon(state.repeat.get());
            repeat.set_icon_name(icon);
            repeat.set_tooltip_text(Some(tip));
            repeat.set_active(on);
        }
    });
    volume.connect_value_changed({
        let (state, mute) = (state.clone(), mute.clone());
        move |s| {
            state.player.set_volume(s.value());
            state.player.set_muted(false);
            mute.set_icon_name(volume_icon(s.value()));
        }
    });
    mute.connect_clicked({
        let (state, volume) = (state.clone(), volume.clone());
        move |b| {
            let muted = !state.player.muted();
            state.player.set_muted(muted);
            b.set_icon_name(if muted {
                "audio-volume-muted-symbolic"
            } else {
                volume_icon(volume.value())
            });
        }
    });
    mini.connect_clicked({
        let (state, app) = (state.clone(), app.clone());
        move |b| {
            let main = b.root().and_downcast::<gtk::Window>();
            let window = mini_window(&state, &app, main.clone());
            if let Some(main) = main {
                main.set_visible(false);
            }
            window.present();
        }
    });
    bar
}

fn volume_icon(value: f64) -> &'static str {
    match value {
        v if v <= 0.0 => "audio-volume-muted-symbolic",
        v if v < 0.34 => "audio-volume-low-symbolic",
        v if v < 0.67 => "audio-volume-medium-symbolic",
        _ => "audio-volume-high-symbolic",
    }
}

/// The compact player: artwork, title, artist, progress and the three
/// buttons. Closing it (or the expand button) brings the library back.
pub fn mini_window(
    state: &Rc<State>,
    app: &adw::Application,
    main: Option<gtk::Window>,
) -> adw::Window {
    let art = artwork(112);
    let title = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["title-4"])
        .build();
    let artist = gtk::Label::builder()
        .xalign(0.0)
        .ellipsize(gtk::pango::EllipsizeMode::End)
        .css_classes(["dim-label"])
        .build();
    let (scale, elapsed, remaining) = progress(state);
    let times = gtk::Box::builder().build();
    elapsed.set_hexpand(true);
    elapsed.set_xalign(0.0);
    times.append(&elapsed);
    times.append(&remaining);
    let previous = flat("media-skip-backward-symbolic", "Zurück");
    let play = play_button();
    let next = flat("media-skip-forward-symbolic", "Weiter");
    let expand = flat("view-fullscreen-symbolic", "Bibliothek zeigen");
    let controls = gtk::Box::builder().spacing(8).build();
    let spacer = gtk::Box::builder().hexpand(true).build();
    controls.append(&previous);
    controls.append(&play);
    controls.append(&next);
    controls.append(&spacer);
    controls.append(&expand);
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(2)
        .hexpand(true)
        .valign(gtk::Align::Center)
        .build();
    for w in [
        title.upcast_ref::<gtk::Widget>(),
        artist.upcast_ref(),
        scale.upcast_ref(),
        times.upcast_ref(),
        controls.upcast_ref(),
    ] {
        text.append(w);
    }
    let content = gtk::Box::builder()
        .spacing(16)
        .margin_top(16)
        .margin_bottom(16)
        .margin_start(16)
        .margin_end(16)
        .build();
    content.append(&art);
    content.append(&text);
    let handle = gtk::WindowHandle::builder().child(&content).build();

    let window = adw::Window::builder()
        .application(app)
        .title("Mini-Player")
        .default_width(420)
        .resizable(false)
        .content(&handle)
        .build();
    window.add_css_class("mini-player");
    show_now_playing(state, &title, &artist, &art, &play);
    connect_controls(state, &previous, &play, &next);

    let restore = {
        let (window, main) = (window.clone(), main.clone());
        move || {
            if let Some(main) = &main {
                main.present();
            }
            window.set_visible(false);
        }
    };
    let restore = Rc::new(restore);
    expand.connect_clicked({
        let restore = restore.clone();
        move |_| restore()
    });
    window.connect_close_request(move |_| {
        restore();
        glib::Propagation::Stop
    });
    window
}
