# daylog-core

Shared data layer for [daylog](https://github.com/Manas-Kenge/Daylog), a terminal screen-time tracker for Linux.

This crate provides the aw-server HTTP client, query layer, aggregations, category-matching engine, KPI math, and time-range types that the `daylog` binary builds on top of.

It is published primarily so that `cargo install daylog` can resolve it from the registry. The public API is shaped by `daylog`'s needs; semver applies but the crate is **not aimed at general use**. If you want to read aw-server data programmatically, [aw-client-rust](https://crates.io/crates/aw-client-rust) is the upstream library.

## License

MIT — see [LICENSE](https://github.com/Manas-Kenge/Daylog/blob/master/LICENSE).
