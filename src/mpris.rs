//! MPRIS 2 on the session bus: the media card in the control centre, the
//! media keys (playerctl) and other tools can see and steer the player.

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

use gtk::gio::prelude::*;
use gtk::{gio, glib};

use crate::state::{Repeat, State};

const PATH: &str = "/org/mpris/MediaPlayer2";
const NAME: &str = "org.mpris.MediaPlayer2.aco_music";
const PLAYER: &str = "org.mpris.MediaPlayer2.Player";
const XML: &str = r#"<node>
  <interface name="org.mpris.MediaPlayer2">
    <method name="Raise"/>
    <method name="Quit"/>
    <property name="CanQuit" type="b" access="read"/>
    <property name="CanRaise" type="b" access="read"/>
    <property name="HasTrackList" type="b" access="read"/>
    <property name="Identity" type="s" access="read"/>
    <property name="DesktopEntry" type="s" access="read"/>
    <property name="SupportedUriSchemes" type="as" access="read"/>
    <property name="SupportedMimeTypes" type="as" access="read"/>
  </interface>
  <interface name="org.mpris.MediaPlayer2.Player">
    <method name="Next"/>
    <method name="Previous"/>
    <method name="Pause"/>
    <method name="PlayPause"/>
    <method name="Stop"/>
    <method name="Play"/>
    <method name="Seek"><arg direction="in" type="x" name="Offset"/></method>
    <method name="SetPosition"><arg direction="in" type="o" name="TrackId"/><arg direction="in" type="x" name="Position"/></method>
    <method name="OpenUri"><arg direction="in" type="s" name="Uri"/></method>
    <signal name="Seeked"><arg type="x" name="Position"/></signal>
    <property name="PlaybackStatus" type="s" access="read"/>
    <property name="LoopStatus" type="s" access="readwrite"/>
    <property name="Rate" type="d" access="readwrite"/>
    <property name="Shuffle" type="b" access="readwrite"/>
    <property name="Metadata" type="a{sv}" access="read"/>
    <property name="Volume" type="d" access="readwrite"/>
    <property name="Position" type="x" access="read"/>
    <property name="MinimumRate" type="d" access="read"/>
    <property name="MaximumRate" type="d" access="read"/>
    <property name="CanGoNext" type="b" access="read"/>
    <property name="CanGoPrevious" type="b" access="read"/>
    <property name="CanPlay" type="b" access="read"/>
    <property name="CanPause" type="b" access="read"/>
    <property name="CanSeek" type="b" access="read"/>
    <property name="CanControl" type="b" access="read"/>
  </interface>
</node>"#;

fn status(state: &State) -> &'static str {
    match (state.current().is_some(), state.player.is_playing()) {
        (false, _) => "Stopped",
        (true, true) => "Playing",
        (true, false) => "Paused",
    }
}

/// The cover as a file, since MPRIS passes artwork as a URL.
fn art_url(state: &State, key: &str) -> Option<String> {
    let bytes = state.cover_bytes.borrow().get(key).cloned()?;
    let mut hasher = DefaultHasher::new();
    key.hash(&mut hasher);
    let dir = aco_common::data_dir(glib::user_cache_dir(), "music");
    let path = dir.join(format!("cover-{:016x}", hasher.finish()));
    if !path.exists() {
        std::fs::write(&path, &bytes[..]).ok()?;
    }
    glib::filename_to_uri(&path, None)
        .ok()
        .map(|u| u.to_string())
}

fn metadata(state: &State) -> glib::Variant {
    let dict = glib::VariantDict::new(None);
    if let (Some(track), Some(position)) = (state.current(), state.position.get()) {
        let id = format!("/org/aco/music/track/{position}");
        if let Ok(path) = glib::variant::ObjectPath::try_from(id) {
            dict.insert_value("mpris:trackid", &path.to_variant());
        }
        dict.insert_value(
            "mpris:length",
            &((track.duration * 1e6) as i64).to_variant(),
        );
        dict.insert_value("xesam:title", &track.title.to_variant());
        dict.insert_value("xesam:artist", &vec![track.artist.clone()].to_variant());
        dict.insert_value("xesam:album", &track.album.to_variant());
        if let Ok(uri) = glib::filename_to_uri(&track.path, None) {
            dict.insert_value("xesam:url", &uri.to_string().to_variant());
        }
        if let Some(url) = art_url(state, &track.album_key()) {
            dict.insert_value("mpris:artUrl", &url.to_variant());
        }
    }
    dict.end()
}

fn property(state: &State, name: &str) -> glib::Variant {
    match name {
        "CanQuit" | "CanRaise" | "CanPlay" | "CanPause" | "CanSeek" | "CanControl"
        | "CanGoNext" | "CanGoPrevious" => true.to_variant(),
        "HasTrackList" => false.to_variant(),
        "Identity" => "Musik".to_variant(),
        "DesktopEntry" => "org.aco.Music".to_variant(),
        "SupportedUriSchemes" => vec!["file"].to_variant(),
        "SupportedMimeTypes" => vec![
            "audio/mpeg",
            "audio/flac",
            "audio/ogg",
            "audio/x-wav",
            "audio/mp4",
        ]
        .to_variant(),
        "PlaybackStatus" => status(state).to_variant(),
        "LoopStatus" => match state.repeat.get() {
            Repeat::Off => "None",
            Repeat::All => "Playlist",
            Repeat::One => "Track",
        }
        .to_variant(),
        "Rate" | "MinimumRate" | "MaximumRate" => 1.0f64.to_variant(),
        "Shuffle" => state.shuffle.get().to_variant(),
        "Metadata" => metadata(state),
        "Volume" => state.player.volume().to_variant(),
        "Position" => ((state.player.position().unwrap_or(0.0) * 1e6) as i64).to_variant(),
        _ => "".to_variant(),
    }
}

/// Publish the player on the session bus. `raise` shows the window.
pub fn publish(state: Rc<State>, app: adw::Application, raise: Rc<dyn Fn()>) {
    let Ok(connection) = gio::bus_get_sync(gio::BusType::Session, gio::Cancellable::NONE) else {
        return;
    };
    let Ok(node) = gio::DBusNodeInfo::for_xml(XML) else {
        return;
    };
    let (Some(root), Some(player)) = (
        node.lookup_interface("org.mpris.MediaPlayer2"),
        node.lookup_interface(PLAYER),
    ) else {
        return;
    };

    let registered = connection
        .register_object(PATH, &root)
        .method_call({
            let raise = raise.clone();
            move |_, _, _, _, method, _, invocation| {
                match method {
                    "Raise" => raise(),
                    "Quit" => app.quit(),
                    _ => {}
                }
                invocation.return_value(None);
            }
        })
        .property({
            let state = state.clone();
            move |_, _, _, _, name| property(&state, name)
        })
        .build();
    if registered.is_err() {
        return;
    }

    let _ = connection
        .register_object(PATH, &player)
        .method_call({
            let state = state.clone();
            let connection = connection.clone();
            move |_, _, _, _, method, params, invocation| {
                match method {
                    "Next" => state.next(),
                    "Previous" => state.previous(),
                    "Pause" => {
                        state.player.pause();
                        state.changed();
                    }
                    "Play" => {
                        if !state.player.is_playing() {
                            state.toggle();
                        }
                    }
                    "PlayPause" => state.toggle(),
                    "Stop" => {
                        state.player.stop();
                        state.changed();
                    }
                    "Seek" => {
                        if let Some((offset,)) = params.get::<(i64,)>() {
                            let target =
                                state.player.position().unwrap_or(0.0) + offset as f64 / 1e6;
                            state.player.seek(target);
                            seeked(&connection, target);
                        }
                    }
                    "SetPosition" => {
                        if let Some((_, position)) =
                            params.get::<(glib::variant::ObjectPath, i64)>()
                        {
                            let target = position as f64 / 1e6;
                            state.player.seek(target);
                            seeked(&connection, target);
                        }
                    }
                    "OpenUri" => {
                        if let Some((uri,)) = params.get::<(String,)>() {
                            crate::open_files(&state, &[gio::File::for_uri(&uri)]);
                        }
                    }
                    _ => {}
                }
                invocation.return_value(None);
            }
        })
        .property({
            let state = state.clone();
            move |_, _, _, _, name| property(&state, name)
        })
        .set_property({
            let state = state.clone();
            move |_, _, _, _, name, value| {
                match name {
                    "Shuffle" => state.set_shuffle(value.get::<bool>().unwrap_or(false)),
                    "Volume" => state.player.set_volume(value.get::<f64>().unwrap_or(1.0)),
                    "LoopStatus" => {
                        let wanted = match value.get::<String>().as_deref() {
                            Some("Playlist") => Repeat::All,
                            Some("Track") => Repeat::One,
                            _ => Repeat::Off,
                        };
                        state.repeat.set(wanted);
                        state.changed();
                    }
                    _ => {}
                }
                true
            }
        })
        .build();

    let _ = gio::bus_own_name_on_connection(
        &connection,
        NAME,
        gio::BusNameOwnerFlags::NONE,
        |_, _| {},
        |_, _| {},
    );

    // Tell listeners when the track, state, shuffle or repeat change.
    state.subscribe({
        let (state, connection) = (state.clone(), connection.clone());
        move || {
            let changed = glib::VariantDict::new(None);
            for name in [
                "PlaybackStatus",
                "Metadata",
                "Shuffle",
                "LoopStatus",
                "Volume",
            ] {
                changed.insert_value(name, &property(&state, name));
            }
            let params = glib::Variant::tuple_from_iter([
                PLAYER.to_variant(),
                changed.end(),
                Vec::<String>::new().to_variant(),
            ]);
            let _ = connection.emit_signal(
                None,
                PATH,
                "org.freedesktop.DBus.Properties",
                "PropertiesChanged",
                Some(&params),
            );
        }
    });
}

fn seeked(connection: &gio::DBusConnection, seconds: f64) {
    let params = glib::Variant::tuple_from_iter([((seconds * 1e6) as i64).to_variant()]);
    let _ = connection.emit_signal(None, PATH, PLAYER, "Seeked", Some(&params));
}
