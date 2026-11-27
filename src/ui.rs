//! The main window: sidebar (all tracks, albums, artists), a header with large
//! artwork, the track list, and the player bar at the bottom.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use aco_common::format_time;
use adw::prelude::*;
use gtk::{gio, glib};

use crate::library::Track;
use crate::state::{Repeat, State};

/// What the track list shows.
#[derive(Clone, PartialEq)]
enum View {
    All,
    Album(String),
    Artist(String),
}

struct Ui {
    state: Rc<State>,
    stack: gtk::Stack,
    store: gio::ListStore,
    /// Library indices of the rows in `store`, for "play from here".
    rows: RefCell<Vec<usize>>,
    view: RefCell<View>,
    search: gtk::SearchEntry,
    back: gtk::Button,
    header_art: gtk::Image,
    header_kind: gtk::Label,
    header_title: gtk::Label,
    header_info: gtk::Label,
    albums: gtk::FlowBox,
    artists: gtk::ListBox,
    playing_path: RefCell<Option<std::path::PathBuf>>,
}

pub fn artwork(size: i32) -> gtk::Image {
    let image = gtk::Image::from_icon_name("audio-x-generic-symbolic");
    image.set_pixel_size(size);
    image.set_size_request(size, size);
    image.add_css_class("artwork");
    image.set_overflow(gtk::Overflow::Hidden);
    image
}

/// Show a track's cover, or the note icon without one.
pub fn set_art(image: &gtk::Image, state: &State, track: Option<&Track>) {
    match track.and_then(|t| state.cover(t)) {
        Some(texture) => image.set_paintable(Some(&texture)),
        None => image.set_icon_name(Some("audio-x-generic-symbolic")),
    }
}

pub fn build(app: &adw::Application, state: Rc<State>) -> (adw::ApplicationWindow, Rc<dyn Fn()>) {
    // --- Sidebar -------------------------------------------------------------------
    let nav = gtk::ListBox::new();
    nav.add_css_class("navigation-sidebar");
    for (icon, label, name) in [
        ("audio-x-generic-symbolic", "Alle Titel", "all"),
        ("media-optical-symbolic", "Alben", "albums"),
        ("system-users-symbolic", "Künstler", "artists"),
    ] {
        let row_box = gtk::Box::builder()
            .spacing(12)
            .margin_top(6)
            .margin_bottom(6)
            .margin_start(6)
            .build();
        row_box.append(&gtk::Image::from_icon_name(icon));
        row_box.append(&gtk::Label::new(Some(label)));
        nav.append(
            &gtk::ListBoxRow::builder()
                .child(&row_box)
                .name(name)
                .build(),
        );
    }
    let sidebar_view = adw::ToolbarView::new();
    sidebar_view.add_top_bar(
        &adw::HeaderBar::builder()
            .show_end_title_buttons(false)
            .build(),
    );
    sidebar_view.set_content(Some(&nav));
    let sidebar = adw::NavigationPage::builder()
        .title("Musik")
        .child(&sidebar_view)
        .build();

    // --- Track list page -------------------------------------------------------------
    let header_art = artwork(168);
    let header_kind = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["caption-heading", "dim-label"])
        .build();
    let header_title = gtk::Label::builder()
        .xalign(0.0)
        .wrap(true)
        .css_classes(["title-1"])
        .build();
    let header_info = gtk::Label::builder()
        .xalign(0.0)
        .css_classes(["dim-label"])
        .build();
    let play_all = gtk::Button::builder()
        .label("Abspielen")
        .css_classes(["pill", "suggested-action"])
        .build();
    let shuffle_all = gtk::Button::builder()
        .label("Zufällig")
        .css_classes(["pill"])
        .build();
    let buttons = gtk::Box::builder().spacing(12).margin_top(12).build();
    buttons.append(&play_all);
    buttons.append(&shuffle_all);
    let text = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .spacing(4)
        .valign(gtk::Align::End)
        .build();
    for w in [
        header_kind.upcast_ref::<gtk::Widget>(),
        header_title.upcast_ref(),
        header_info.upcast_ref(),
        buttons.upcast_ref(),
    ] {
        text.append(w);
    }
    let header = gtk::Box::builder()
        .spacing(24)
        .margin_top(24)
        .margin_bottom(18)
        .margin_start(24)
        .margin_end(24)
        .build();
    header.append(&header_art);
    header.append(&text);

    let store = gio::ListStore::new::<glib::BoxedAnyObject>();
    let selection = gtk::SingleSelection::builder()
        .model(&store)
        .autoselect(false)
        .can_unselect(true)
        .build();
    let factory = gtk::SignalListItemFactory::new();
    let list = gtk::ListView::builder()
        .model(&selection)
        .factory(&factory)
        .css_classes(["track-list"])
        .build();
    let tracks_page = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    tracks_page.append(&header);
    tracks_page.append(
        &gtk::ScrolledWindow::builder()
            .child(&list)
            .vexpand(true)
            .build(),
    );

    // --- Albums and artists pages ------------------------------------------------------------
    let albums = gtk::FlowBox::builder()
        .selection_mode(gtk::SelectionMode::None)
        .homogeneous(true)
        .max_children_per_line(8)
        .column_spacing(18)
        .row_spacing(18)
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .valign(gtk::Align::Start)
        .build();
    let artists = gtk::ListBox::builder()
        .css_classes(["boxed-list"])
        .margin_top(24)
        .margin_bottom(24)
        .margin_start(24)
        .margin_end(24)
        .valign(gtk::Align::Start)
        .build();
    let empty = adw::StatusPage::builder()
        .icon_name("audio-x-generic-symbolic")
        .title("Keine Musik gefunden")
        .description("Musik im Ordner „Musik“ ablegen oder Dateien öffnen.")
        .build();

    let stack = gtk::Stack::builder()
        .transition_type(gtk::StackTransitionType::Crossfade)
        .build();
    stack.add_named(&tracks_page, Some("tracks"));
    stack.add_named(
        &gtk::ScrolledWindow::builder().child(&albums).build(),
        Some("albums"),
    );
    stack.add_named(
        &gtk::ScrolledWindow::builder().child(&artists).build(),
        Some("artists"),
    );
    stack.add_named(&empty, Some("empty"));

    let search = gtk::SearchEntry::builder()
        .placeholder_text("Titel, Künstler oder Album")
        .width_chars(28)
        .build();
    let back = aco_common::icon_button("go-previous-symbolic", "Zurück");
    back.set_visible(false);
    let open = aco_common::icon_button("document-open-symbolic", "Dateien öffnen");
    let content_header = adw::HeaderBar::new();
    content_header.pack_start(&back);
    content_header.set_title_widget(Some(&search));
    content_header.pack_end(&open);
    let content_view = adw::ToolbarView::new();
    content_view.add_top_bar(&content_header);
    let toasts = adw::ToastOverlay::new();
    toasts.set_child(Some(&stack));
    content_view.set_content(Some(&toasts));
    let content = adw::NavigationPage::builder()
        .title("Musik")
        .child(&content_view)
        .build();

    let split = adw::NavigationSplitView::builder()
        .sidebar(&sidebar)
        .content(&content)
        .min_sidebar_width(200.0)
        .max_sidebar_width(240.0)
        .build();

    let ui = Rc::new(Ui {
        state: state.clone(),
        stack,
        store,
        rows: RefCell::default(),
        view: RefCell::new(View::All),
        search,
        back,
        header_art,
        header_kind,
        header_title,
        header_info,
        albums,
        artists,
        playing_path: RefCell::default(),
    });

    // Rows: cover, title over artist, album, duration.
    factory.connect_setup(|_, item| {
        let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
        let row = gtk::Box::builder()
            .spacing(14)
            .margin_top(4)
            .margin_bottom(4)
            .margin_start(18)
            .margin_end(18)
            .build();
        row.append(&artwork(40));
        let names = gtk::Box::builder()
            .orientation(gtk::Orientation::Vertical)
            .valign(gtk::Align::Center)
            .width_request(280)
            .build();
        names.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["heading"])
                .build(),
        );
        names.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["dim-label", "caption"])
                .build(),
        );
        row.append(&names);
        row.append(
            &gtk::Label::builder()
                .xalign(0.0)
                .hexpand(true)
                .ellipsize(gtk::pango::EllipsizeMode::End)
                .css_classes(["dim-label"])
                .build(),
        );
        row.append(
            &gtk::Label::builder()
                .xalign(1.0)
                .css_classes(["dim-label", "numeric"])
                .build(),
        );
        item.set_child(Some(&row));
    });
    factory.connect_bind({
        let ui = ui.clone();
        move |_, item| {
            let item = item.downcast_ref::<gtk::ListItem>().expect("list item");
            let (Some(row), Some(obj)) = (
                item.child(),
                item.item().and_downcast::<glib::BoxedAnyObject>(),
            ) else {
                return;
            };
            let index = *obj.borrow::<usize>();
            let tracks = ui.state.tracks.borrow();
            let Some(track) = tracks.get(index) else {
                return;
            };
            let image = row
                .first_child()
                .and_downcast::<gtk::Image>()
                .expect("cover");
            let names = image.next_sibling().expect("names");
            let album = names
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .expect("album");
            let duration = album
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .expect("duration");
            let title = names
                .first_child()
                .and_downcast::<gtk::Label>()
                .expect("title");
            let artist = title
                .next_sibling()
                .and_downcast::<gtk::Label>()
                .expect("artist");
            set_art(&image, &ui.state, Some(track));
            title.set_label(&track.title);
            artist.set_label(&track.artist);
            album.set_label(&track.album);
            duration.set_label(&format_time(track.duration));
            if ui.playing_path.borrow().as_ref() == Some(&track.path) {
                title.add_css_class("accent");
            } else {
                title.remove_css_class("accent");
            }
        }
    });
    list.connect_activate({
        let ui = ui.clone();
        move |_, position| {
            let rows = ui.rows.borrow().clone();
            ui.state.play_list(rows, position as usize);
        }
    });

    // --- Navigation -------------------------------------------------------------------------------
    nav.connect_row_selected({
        let ui = ui.clone();
        move |_, row| {
            let Some(row) = row else { return };
            match row.widget_name().as_str() {
                "albums" => ui.show_page("albums"),
                "artists" => ui.show_page("artists"),
                _ => ui.show_tracks(View::All),
            }
        }
    });
    ui.back.connect_clicked({
        let (ui, nav) = (ui.clone(), nav.clone());
        move |_| {
            let page = if matches!(*ui.view.borrow(), View::Artist(_)) {
                "artists"
            } else {
                "albums"
            };
            ui.show_page(page);
            if let Some(row) = nav.row_at_index(if page == "albums" { 1 } else { 2 }) {
                nav.select_row(Some(&row));
            }
        }
    });
    ui.search.connect_search_changed({
        let ui = ui.clone();
        move |_| ui.fill_tracks()
    });
    play_all.connect_clicked({
        let ui = ui.clone();
        move |_| {
            ui.state.shuffle.set(false);
            let rows = ui.rows.borrow().clone();
            ui.state.play_list(rows, 0);
        }
    });
    shuffle_all.connect_clicked({
        let ui = ui.clone();
        move |_| {
            ui.state.shuffle.set(true);
            let rows = ui.rows.borrow().clone();
            let start = glib::random_int_range(0, rows.len().max(1) as i32) as usize;
            ui.state.play_list(rows, start);
        }
    });

    // --- Window -----------------------------------------------------------------------------------
    let bar = crate::bar::player_bar(state.clone(), app);
    let body = gtk::Box::builder()
        .orientation(gtk::Orientation::Vertical)
        .build();
    body.append(&split);
    split.set_vexpand(true);
    body.append(&bar);

    let window = adw::ApplicationWindow::builder()
        .application(app)
        .title("Musik")
        .default_width(1120)
        .default_height(740)
        .content(&body)
        .build();
    window.set_size_request(360, 360);
    let breakpoint = adw::Breakpoint::new(adw::BreakpointCondition::new_length(
        adw::BreakpointConditionLengthType::MaxWidth,
        720.0,
        adw::LengthUnit::Sp,
    ));
    breakpoint.add_setter(&split, "collapsed", Some(&true.to_value()));
    window.add_breakpoint(breakpoint);

    open.connect_clicked({
        let (window, ui) = (window.clone(), ui.clone());
        move |_| {
            let ui = ui.clone();
            aco_common::open_files(&window, "Musik öffnen", &["audio/*"], move |files| {
                crate::open_files(&ui.state, &files)
            });
        }
    });

    // The playing title is highlighted in the list: swap in fresh row objects
    // for the same tracks, which makes the list bind every row again.
    state.subscribe({
        let ui = ui.clone();
        move || {
            let now = ui.state.current().map(|t| t.path);
            if *ui.playing_path.borrow() != now {
                *ui.playing_path.borrow_mut() = now;
                let objects: Vec<glib::BoxedAnyObject> = ui.rows.borrow().iter().map(|&i| glib::BoxedAnyObject::new(i)).collect();
                ui.store.splice(0, ui.store.n_items(), &objects);
            }
        }
    });

    let refresh: Rc<dyn Fn()> = Rc::new({
        let ui = ui.clone();
        move || {
            ui.fill_albums();
            ui.fill_artists();
            let view = ui.view.borrow().clone();
            ui.show_tracks(view);
        }
    });
    if let Some(first) = nav.row_at_index(0) {
        nav.select_row(Some(&first));
    }
    (window, refresh)
}

impl Ui {
    fn show_page(&self, name: &str) {
        self.back.set_visible(false);
        self.search.set_visible(false);
        self.stack
            .set_visible_child_name(if self.state.tracks.borrow().is_empty() {
                "empty"
            } else {
                name
            });
    }

    fn show_tracks(&self, view: View) {
        self.back.set_visible(view != View::All);
        self.search.set_visible(view == View::All);
        *self.view.borrow_mut() = view;
        self.fill_tracks();
        self.stack
            .set_visible_child_name(if self.state.tracks.borrow().is_empty() {
                "empty"
            } else {
                "tracks"
            });
    }

    /// Fill the list for the current view and search text, and its header.
    fn fill_tracks(&self) {
        let query = self.search.text().to_lowercase();
        let tracks = self.state.tracks.borrow();
        let view = self.view.borrow().clone();
        let rows: Vec<usize> = tracks
            .iter()
            .enumerate()
            .filter(|(_, t)| match &view {
                View::All => true,
                View::Album(key) => t.album_key() == *key,
                View::Artist(name) => t.artist == *name,
            })
            .filter(|(_, t)| {
                query.is_empty()
                    || t.title.to_lowercase().contains(&query)
                    || t.artist.to_lowercase().contains(&query)
                    || t.album.to_lowercase().contains(&query)
            })
            .map(|(i, _)| i)
            .collect();
        let total: f64 = rows.iter().map(|&i| tracks[i].duration).sum();
        let first = rows.first().map(|&i| tracks[i].clone());
        let (kind, title) = match &view {
            View::All => ("BIBLIOTHEK", "Alle Titel".to_owned()),
            View::Album(_) => (
                "ALBUM",
                first.as_ref().map(|t| t.album.clone()).unwrap_or_default(),
            ),
            View::Artist(name) => ("KÜNSTLER", name.clone()),
        };
        self.header_kind.set_label(kind);
        self.header_title.set_label(&title);
        let minutes = (total / 60.0).round() as u64;
        let length = if minutes >= 60 {
            format!("{} Std. {} Min.", minutes / 60, minutes % 60)
        } else {
            format!("{minutes} Min.")
        };
        let by = match &view {
            View::Album(_) => first
                .as_ref()
                .map(|t| format!("{} · ", t.artist))
                .unwrap_or_default(),
            _ => String::new(),
        };
        self.header_info
            .set_label(&format!("{by}{} Titel · {length}", rows.len()));
        set_art(&self.header_art, &self.state, first.as_ref());
        drop(tracks);

        let objects: Vec<glib::BoxedAnyObject> =
            rows.iter().map(|&i| glib::BoxedAnyObject::new(i)).collect();
        self.store.remove_all();
        self.store.extend_from_slice(&objects);
        *self.rows.borrow_mut() = rows;
    }

    fn fill_albums(self: &Rc<Self>) {
        while let Some(child) = self.albums.first_child() {
            self.albums.remove(&child);
        }
        let tracks = self.state.tracks.borrow();
        let mut seen = std::collections::HashSet::new();
        for track in tracks.iter() {
            let key = track.album_key();
            if !seen.insert(key.clone()) {
                continue;
            }
            let card = gtk::Box::builder()
                .orientation(gtk::Orientation::Vertical)
                .spacing(6)
                .build();
            let art = artwork(160);
            set_art(&art, &self.state, Some(track));
            card.append(&art);
            card.append(
                &gtk::Label::builder()
                    .label(&track.album)
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(18)
                    .css_classes(["heading"])
                    .build(),
            );
            card.append(
                &gtk::Label::builder()
                    .label(&track.artist)
                    .xalign(0.0)
                    .ellipsize(gtk::pango::EllipsizeMode::End)
                    .max_width_chars(18)
                    .css_classes(["dim-label", "caption"])
                    .build(),
            );
            let button = gtk::Button::builder()
                .child(&card)
                .css_classes(["flat", "album-card"])
                .build();
            let ui = self.clone();
            button.connect_clicked(move |_| ui.show_tracks(View::Album(key.clone())));
            self.albums.append(&button);
        }
    }

    fn fill_artists(self: &Rc<Self>) {
        while let Some(child) = self.artists.first_child() {
            self.artists.remove(&child);
        }
        let tracks = self.state.tracks.borrow();
        let mut counts: Vec<(String, usize)> = Vec::new();
        for track in tracks.iter() {
            match counts.iter_mut().find(|(name, _)| *name == track.artist) {
                Some((_, n)) => *n += 1,
                None => counts.push((track.artist.clone(), 1)),
            }
        }
        counts.sort_by_key(|(name, _)| name.to_lowercase());
        for (name, count) in counts {
            let row = adw::ActionRow::builder()
                .title(&name)
                .subtitle(format!("{count} Titel"))
                .activatable(true)
                .build();
            let avatar = adw::Avatar::new(40, Some(&name), true);
            row.add_prefix(&avatar);
            row.add_suffix(&gtk::Image::from_icon_name("go-next-symbolic"));
            let ui = self.clone();
            row.connect_activated(move |_| ui.show_tracks(View::Artist(name.clone())));
            self.artists.append(&row);
        }
    }
}

/// Labels and icon of the repeat button for the current mode.
pub fn repeat_icon(repeat: Repeat) -> (&'static str, &'static str, bool) {
    match repeat {
        Repeat::Off => ("media-playlist-repeat-symbolic", "Wiederholen: aus", false),
        Repeat::All => ("media-playlist-repeat-symbolic", "Alle wiederholen", true),
        Repeat::One => (
            "media-playlist-repeat-song-symbolic",
            "Titel wiederholen",
            true,
        ),
    }
}

/// Keep a progress slider and two time labels in step with the player.
pub fn follow_progress(
    state: Rc<State>,
    scale: gtk::Scale,
    elapsed: gtk::Label,
    remaining: gtk::Label,
) {
    let dragging = Rc::new(Cell::new(false));
    scale.connect_change_value({
        let (state, dragging) = (state.clone(), dragging.clone());
        move |_, _, value| {
            dragging.set(false);
            state.player.seek(value);
            glib::Propagation::Proceed
        }
    });
    glib::timeout_add_local(std::time::Duration::from_millis(500), move || {
        let position = state.player.position().unwrap_or(0.0);
        let duration = state.player.duration().unwrap_or(0.0);
        if !dragging.get() {
            scale.set_range(0.0, duration.max(1.0));
            scale.set_value(position);
        }
        elapsed.set_label(&format_time(position));
        remaining.set_label(&format!("−{}", format_time((duration - position).max(0.0))));
        glib::ControlFlow::Continue
    });
}
