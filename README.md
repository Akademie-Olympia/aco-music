# ACO Musik (`aco-music`)

The music library and player of the ACO desktop: reads the music folder (tags and covers with lofty), albums, artists and search, a player bar and a compact mini player, and MPRIS, so the control centre's media card and the media keys steer it. Press Ctrl+F to search the library from any view. The initial library scan keeps already queued files in order.

Native app in Rust with GTK 4 and libadwaita: it follows the ACO theme (profile, light/dark, accent) through
aco-gtk4 without code of its own, starts on demand and keeps nothing in the background. Shared parts (app start,
dialogs, the GStreamer player) come from [aco-common](https://github.com/Akademie-Olympia/aco-common).
Part of the [ACO media tools](https://github.com/Akademie-Olympia/aco-media-tools).

```bash
aco-music [FILE…]                 # open files, or start empty
cargo build --release          # needs gtk4 and libadwaita (the normal Arch packages)
```

Package: `aco-arch-dotfiles-music` (`packaging/arch/PKGBUILD`, built from source with cargo following the Arch Rust
package guidelines). [MIT licence](LICENSE).
