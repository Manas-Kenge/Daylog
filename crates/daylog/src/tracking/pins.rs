//! Pinned upstream binaries; bumps must update scripts/binaries.lock too.
//! Schema coupling: datastore.rs targets aw-server-rust user_version=4.

pub(crate) struct BinaryPin {
    pub name: &'static str,
    pub url: &'static str,
    pub archive_sha256: &'static str,
    pub extract: Extraction,
}

pub(crate) enum Extraction {
    OneFromZip { archive_path: &'static str },
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

pub(crate) const GNOME_EXTENSION: BinaryPin = BinaryPin {
    name: "focused-window-dbus@flexagoon.com.zip",
    url: "https://extensions.gnome.org/download-extension/focused-window-dbus@flexagoon.com.shell-extension.zip?version_tag=62865",
    archive_sha256: "2c9ca5d737f7fe64197f44727d5391a5fcac71ebafe87b820967a844df716770",
    extract: Extraction::WholeZip,
};
