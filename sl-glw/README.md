# sl-glw

sl-glw:
[![Crates.io Version sl-glw][crates-img]][crates-link]
[![lib.rs Version sl-glw][libs-img]][libs-link]
[![docs.rs sl-glw][docs-img]][docs-link]
[![Dependency status sl-glw][deps-img]][deps-link]

Fetch, parse, and render [GlobalWind][glw] (GLW) wind/current/wave event
overlays for Second Life sailing maps.

GlobalWind is a community weather system for Second Life sailing: a GLW
"event" describes a base wind/current/wave state for an area of the grid,
plus per-region rectangular *areas* and *circles* that override it. This
crate fetches an event from a GLW server, parses its JSON into strongly
typed values, and draws the wind arrows, current arrows, wave glyphs,
override labels, and an optional legend onto a Second Life map image.

It builds on [`sl-map-apis`][sl-map-apis] (the `MapLike` map image and grid
geometry) and [`sl-types`][sl-types] (grid coordinates and zoom levels).

## What it gives you

- **Typed model** of a GLW event (`GlwEvent`, `Base`, `Area`, `Circle`, and
  the per-quantity newtypes like `WindDirection`, `KnotSpeed`, `WaveHeight`)
  with validating constructors that reject out-of-range values.
- **Fetching** by numeric id or string key, either through the low-level
  `GlwClient` / `fetch_event_by_*` functions or the caching `GlwEventCache`.
- **HTTP caching** via `http-cache-semantics`, layered as an in-memory LRU
  over a persistent on-disk [`redb`] store, so repeated lookups respect the
  server's cache headers instead of re-fetching.
- **Rendering** through the `MapLikeGlwExt` extension trait, which adds
  `draw_glw_event` / `draw_glw_event_with_font` (and per-shape helpers) to
  any `sl-map-apis` `MapLike`. Appearance is controlled by `GlwStyle` and
  `GlwColorPalette`.

## Fonts

The crate **never bundles a font.** Label and legend rendering takes any
[`ab_glyph::Font`] you supply, so you choose the typeface and licensing.
The drawing methods that emit text (`draw_glw_event_with_font` and the
`*_label` / `*_legend` helpers) require a font argument; `draw_glw_event`
draws only the geometric overlay and needs none. A copy of DejaVuSans is
checked in at the workspace root for examples and tests.

## Example

Fetch an event and draw it onto a map. `GlwEventCache::new` takes a cache
directory and an optional base URL (`None` uses the GLW default); the
`MapLike` here is whatever `sl-map-apis` map you are rendering onto.

```rust,ignore
use sl_glw::{EventId, GlwEventCache, GlwStyle, MapLikeGlwExt as _};
use sl_map_apis::coverage::PlacementSlot;

// Persistent cache under ./cache, default GLW server.
let mut cache = GlwEventCache::new("cache".into(), None)?;

// Returns Ok(None) if the server reports no such event.
if let Some(event) = cache.get_event_by_id(EventId::new(6910)).await? {
    let style = GlwStyle {
        legend_position: Some(PlacementSlot::TopLeft),
        ..GlwStyle::default()
    };

    // The caller supplies the font; the library never does.
    let bytes = std::fs::read("DejaVuSans.ttf")?;
    let font = ab_glyph::FontVec::try_from_vec(bytes)?;

    map.draw_glw_event_with_font(&event, &style, &font)?;
}
```

For a complete, runnable program — including a minimal in-memory `MapLike`
that needs no map tiles — see [`examples/render_sample.rs`][example]:

```text
cargo run -p sl-glw --example render_sample -- DejaVuSans.ttf
```

The [`sl-map-cli`][sl-map-cli] and [`sl-map-web`][sl-map-web] crates in the
same workspace show the overlay wired into real map renders.

## License

Licensed under either of the Apache License, Version 2.0 or the MIT license
at your option (`MIT OR Apache-2.0`).

[glw]: http://globalwind.net
[sl-map-apis]: https://crates.io/crates/sl-map-apis
[sl-types]: https://crates.io/crates/sl-types
[sl-map-cli]: https://crates.io/crates/sl-map-cli
[sl-map-web]: https://github.com/taladar/sl-map-tools/tree/master/sl-map-web
[example]: https://github.com/taladar/sl-map-tools/blob/master/sl-glw/examples/render_sample.rs
[`redb`]: https://crates.io/crates/redb
[`ab_glyph::Font`]: https://docs.rs/ab_glyph/latest/ab_glyph/trait.Font.html

[crates-img]: https://img.shields.io/crates/v/sl-glw
[crates-link]: https://crates.io/crates/sl-glw
[libs-img]: https://img.shields.io/crates/v/sl-glw?label=lib.rs
[libs-link]: https://lib.rs/crates/sl-glw
[docs-img]: https://img.shields.io/docsrs/sl-glw
[docs-link]: https://docs.rs/sl-glw/latest/sl-glw
[deps-img]: https://deps.rs/crate/sl-glw/latest/status.svg
[deps-link]: https://deps.rs/crate/sl-glw/
