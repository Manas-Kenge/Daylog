//! Pinned upstream binaries the wizard fetches on first launch.
//!
//! Keep this file in lockstep with `scripts/binaries.lock`. When you bump
//! a pin, update the version + sha256 here AND in the lockfile so the
//! `scripts/fetch-binaries.sh` developer cache pre-warmer stays valid.
//!
//! **Schema coupling**: daylog reads aw-server-rust's SQLite store
//! directly (see `daylog_core::datastore`). The current pin targets
//! schema `user_version=4`. If a future bump alters the events/buckets
//! table layout, `crates/daylog-core/src/datastore.rs` needs to follow
//! before this pin lands.
//!
//! The crate ships ~5 MB instead of ~50 MB by keeping these out of the
//! source tarball — they're downloaded into `~/.cache/daylog/binaries/`
//! on first launch, sha256-verified, then extracted into
//! `~/.local/share/daylog/bin/`.

/// One downloadable artifact (a zip archive). May contain a single binary
/// to extract (the upstream awatcher zip), or be its own artifact (a GNOME
/// shell extension zip that `gnome-extensions install` consumes whole).
pub(crate) struct BinaryPin {
    /// Display name, also the on-disk filename for `OneFromZip` artifacts.
    pub name: &'static str,
    pub url: &'static str,
    /// sha256 of the downloaded archive bytes (lowercase hex).
    pub archive_sha256: &'static str,
    pub extract: Extraction,
}

pub(crate) enum Extraction {
    /// Pull a single named entry out of the zip and place it at
    /// `<bin_dir>/<pin.name>`, executable.
    OneFromZip { archive_path: &'static str },
    /// The zip itself is the artifact (used for GNOME shell extensions —
    /// `gnome-extensions install <zip>` handles the rest).
    WholeZip,
}

pub(crate) const TRACKER_BINARIES: &[BinaryPin] = &[
    BinaryPin {
        name: "aw-server-rust",
        // aw-server-rust ships inside the ActivityWatch parent bundle; the
        // upstream aw-server-rust repo has no own releases.
        url: "https://github.com/ActivityWatch/activitywatch/releases/download/v0.13.2/activitywatch-v0.13.2-linux-x86_64.zip",
        archive_sha256: "8f62b10babf8a8f108cbdf7267c02fbc1ce2a970fa9535f230b3416b803e3360",
        extract: Extraction::OneFromZip {
            archive_path: "activitywatch/aw-server-rust/aw-server-rust",
        },
    },
    BinaryPin {
        name: "aw-awatcher",
        url: "https://github.com/2e3s/awatcher/releases/download/v0.3.3/aw-awatcher.zip",
        archive_sha256: "30b51a94956e3490d9248a40a8deb6fc7eea9c041a1862ed4c4b23a3e4b633df",
        extract: Extraction::OneFromZip { archive_path: "aw-awatcher" },
    },
];

/// Optional GNOME Wayland shell extension. Only fetched when the wizard
/// detects GNOME-Wayland AND the host has `gnome-extensions` on PATH.
pub(crate) const GNOME_EXTENSION: BinaryPin = BinaryPin {
    name: "focused-window-dbus@flexagoon.com.zip",
    url: "https://extensions.gnome.org/download-extension/focused-window-dbus@flexagoon.com.shell-extension.zip?version_tag=62865",
    archive_sha256: "2c9ca5d737f7fe64197f44727d5391a5fcac71ebafe87b820967a844df716770",
    extract: Extraction::WholeZip,
};
