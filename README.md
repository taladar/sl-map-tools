# SL map tools

[![GitHub Actions Workflow Status](https://img.shields.io/github/actions/workflow/status/taladar/sl-map-tools/github-release.yaml)](https://github.com/taladar/sl-map-tools/actions/workflows/github-release.yaml)

sl-types:
[![Crates.io Version sl-types](https://img.shields.io/crates/v/sl-types)](https://crates.io/crates/sl-types)
[![lib.rs Version sl-types](https://img.shields.io/crates/v/sl-types?label=lib.rs)](https://lib.rs/crates/sl-types)
[![docs.rs sl-types](https://img.shields.io/docsrs/sl-types)](https://docs.rs/sl-types/latest/sl-types)
[![Dependency status sl-types](https://deps.rs/crate/sl-types/latest/status.svg)](https://deps.rs/crate/sl-types/)

sl-map-apis:
[![Crates.io Version sl-map-apis](https://img.shields.io/crates/v/sl-map-apis)](https://crates.io/crates/sl-map-apis)
[![lib.rs Version sl-map-apis](https://img.shields.io/crates/v/sl-map-apis?label=lib.rs)](https://lib.rs/crates/sl-map-apis)
[![docs.rs sl-map-apis](https://img.shields.io/docsrs/sl-map-apis)](https://docs.rs/sl-map-apis/latest/sl-map-apis)
[![Dependency status sl-map-apis](https://deps.rs/crate/sl-map-apis/latest/status.svg)](https://deps.rs/crate/sl-map-apis/)

sl-map-cli:
[![Crates.io Version sl-map-cli](https://img.shields.io/crates/v/sl-map-cli)](https://crates.io/crates/sl-map-cli)
[![lib.rs Version sl-map-cli](https://img.shields.io/crates/v/sl-map-cli?label=lib.rs)](https://lib.rs/crates/sl-map-cli)
[![docs.rs sl-map-cli - none for binary crate](https://img.shields.io/badge/docs-none_for_binary_crate-lightgrey)
[![Dependency status sl-map-cli](https://deps.rs/crate/sl-map-cli/latest/status.svg)](https://deps.rs/crate/sl-map-cli/)

sl-chat-log-parser:
[![Crates.io Version sl-chat-log-parser](https://img.shields.io/crates/v/sl-chat-log-parser)](https://crates.io/crates/sl-chat-log-parser)
[![lib.rs Version sl-chat-log-parser](https://img.shields.io/crates/v/sl-chat-log-parser?label=lib.rs)](https://lib.rs/crates/sl-chat-log-parser)
[![docs.rs sl-chat-log-parser](https://img.shields.io/docsrs/sl-chat-log-parser)](https://docs.rs/sl-chat-log-parser/latest/sl-chat-log-parser)
[![Dependency status sl-chat-log-parser](https://deps.rs/crate/sl-chat-log-parser/latest/status.svg)](https://deps.rs/crate/sl-chat-log-parser/)

This is a small set of Rust libraries and a CLI to generate
Second Life maps using the map tile CDN and the APIs for
resolving region names to grid coordinates and vice versa.

It also includes code to parse USB notecards as used in the sailing
and flying communities by many HUDs in SL.

The APIs and map tiles are cached locally according to the cache headers
presented by the server.

You can use Preferences->Move & View->Map & Minimap->Show grid coordinates on
the worldmap in the Firestorm viewer (and probably others) to show grid
coordinates, Da Boom, the oldest region, is at (1000, 1000).

## Generate a map from a grid rectangle

To generate a map for a given rectangle of grid coordinates you need the
coordinates for the lower left corner (the lowest coordinates on each axis)
and for the upper right corner (the highest coordinates on each axis).

You also should specify the same cache directory on every call and give an
output filename as well as a limit on the image width and height in pixels.

The tool automatically adjusts the resolution down to exactly match the number
of regions in the grid rectangle at the most detailed zoom level that fits into
these maximum dimensions.

By default missing map tiles (as in map tiles where no regions exist in that
area) will appear as black while missing regions inside lower detail map tiles
are shown in a water-like color (#19485a and sometimes close ones due to
JPEG compression) by Linden Lab.

It is possible to change the fill color for missing map tiles with the
--missing-map-tile-color option.

It is also possible to fill the missing regions within map tiles though
determining which regions exist there has some performance impact.

To use this you can use the --missing-region-color option.

```shell
sl_map_cli --cache-dir cache \
  from-grid-rectangle \
  --lower-left-x 380 \
  --lower-left-y 380 \
  --upper-right-x 1500 \
  --upper-right-y 1500 \
  --max-width 2048 \
  --max-height 2048 \
  --output-file test_map.jpg
```

If you want to use the generated map with a PPS HUD you can use the string
printed at the end as the description of the dot prim of your PPS HUD to avoid
having the manually (and less accurately) calibrate the HUD to fit
the map texture.

You can then use a long click on the dot to reset the scripts.

You can also use the aspect ratio printed to edit the PPS HUD to be the correct
size.

## Generate a map from a USB notecard

The USB notecard should be saved into a text file. A hex color code
should be specified for the route waypoints and lines (no arrows or
B-splines yet but that is one of my future plans).

All the notes on cache dir, output file and resolution apply equally to this
mode

```shell
sl_map_cli --cache-dir cache \
  from-usb-notecard \
  --usb-notecard usb_notecard.txt \
  --color '#0f0' \
  --max-width 2048 \
  --max-height 2048 \
  --output-file test_map.jpg
```

It is also possible to additionally save the map without the route to a separate
file by specifying the --output-file-without-route option with another file
name.

## Output format

Both map-producing subcommands take an optional `--format` flag (`png` or
`jpeg`, default `png`). The format is honoured regardless of the output file's
extension, so `--output-file out.dat --format jpeg` produces a JPEG.

```shell
sl_map_cli --cache-dir cache \
  from-grid-rectangle \
  --lower-left-x 1000 --lower-left-y 1000 \
  --upper-right-x 1010 --upper-right-y 1010 \
  --max-width 2048 --max-height 2048 \
  --output-file out.jpg --format jpeg
```

## Per-region annotations

The `--region-rectangles`, `--region-names` and `--region-coordinates` flags
(available on both map subcommands) draw, per region, a hairline white outline,
the region name in its lower-left corner, and/or its `(x, y)` grid coordinates
above the name. The name and coordinate text needs a font: pass `--font` (used
for everything) or override it just for the region text with `--region-font`.

The text is only drawn when each region renders at least 64 pixels across and
the render covers at most 1024 regions; below that the text would not fit or
would fan out into thousands of region-name lookups, so it is skipped (the
rectangle outline is still drawn). The name lookups are cached on disk.

```shell
sl_map_cli --cache-dir cache --font DejaVuSans.ttf \
  from-grid-rectangle \
  --lower-left-x 1000 --lower-left-y 1000 \
  --upper-right-x 1004 --upper-right-y 1004 \
  --max-width 2048 --max-height 2048 \
  --output-file out.png \
  --region-rectangles --region-names --region-coordinates
```

## GLW wind/current/wave overlay

A GLW (GlobalWind) event overlay can be drawn on top of any map. Select the
event with one of `--glw-event-id <id>`, `--glw-event-key <key>` (both fetched
through the cache directory) or `--glw-input-file <json>` (a previously-saved
event). Pass `--glw-output-file <json>` to save the fetched event for offline
reruns. A `--font` is required whenever a GLW overlay is drawn (for the
per-shape labels and the legend).

The visual style can be tuned with the `--glw-*-color` flags (area/circle/margin
outlines, wind/current/wave, area fill, and the per-shape label colour with
`--glw-label-color`), and `--glw-margin-band` draws the dashed override
blending-zone boundary.

The base legend is placed in the top-left corner by default. Use
`--glw-legend-slot <slot>` to move it to any of the nine placement slots (see
below), or `--glw-legend-slot none` to hide it.

```shell
sl_map_cli --cache-dir cache --font DejaVuSans.ttf \
  from-usb-notecard \
  --usb-notecard usb_notecard.txt \
  --max-width 2048 --max-height 2048 \
  --output-file out.png \
  --glw-event-id 6910 --glw-legend-slot bottom_right
```

## Logos and text labels

Logo images and arbitrary multi-line text labels can be placed onto the map at
fixed anchor positions. Each is given as one JSON object, and the `--logo` and
`--label` flags may be repeated to place several.

A logo object (`--logo`) has these fields:

| field      | required | meaning                                              |
| ---------- | -------- | ---------------------------------------------------- |
| `file`     | yes      | path to the logo image (PNG, JPEG or WebP)           |
| `slot`     | yes      | placement slot name (see below)                      |
| `scale`    | no       | integer upscale factor `1`, `2` or `4` (default `1`) |
| `h_align`  | no       | `left` / `center` / `right` within the free area     |
| `v_align`  | no       | `top` / `center` / `bottom` within the free area     |

A label object (`--label`) has these fields:

| field      | required | meaning                                              |
| ---------- | -------- | ---------------------------------------------------- |
| `slot`     | yes      | placement slot name (see below)                      |
| `lines`    | yes      | array of text lines                                  |
| `font_px`  | yes      | font size in pixels (positive)                       |
| `font`     | no       | TrueType font path for this label (default `--font`) |
| `color`    | no       | hex text colour (default `#ffffff`)                  |
| `h_align`  | no       | `left` / `center` / `right`                          |
| `v_align`  | no       | `top` / `center` / `bottom`                          |

```shell
sl_map_cli --cache-dir cache --font DejaVuSans.ttf \
  from-usb-notecard \
  --usb-notecard usb_notecard.txt \
  --max-width 2048 --max-height 2048 \
  --output-file out.png \
  --label '{"slot":"top_right","font_px":28,"color":"#ffffff",
            "lines":["RRMC","2026-04-18"]}' \
  --logo '{"file":"sl_sailing_logo.png",
           "slot":"bottom_left+bottom_center","scale":2}'
```

A placement is rejected (and nothing is rendered) if the logo or text does not
fit the free area at its slot, if two placements target the same slot, or if a
placement clashes with the GLW legend slot.

### Placement slots

There are nine anchor positions, named:

```text
top_left      top_center      top_right
middle_left   center          middle_right
bottom_left   bottom_center   bottom_right
```

Each occupies a third of the image. By default content is aligned outward (e.g.
`top_left` hugs the top-left corner, `center` is centred). Several slots can be
combined into one larger rectangle by joining their names with `+`
(e.g. `bottom_left+bottom_center`); the combined slots **must form a solid
rectangle**, so `top_left+top_center` is valid but `top_left+bottom_right`
(a diagonal) and `top_left+bottom_left` (a gap in the middle) are not.

## Inspecting free placement slots

The `placement-slots` subcommand reports which slots are free for a given
render without producing an image, which helps decide where to put a logo or
label. Describe the area with either the four grid-rectangle corner flags or a
`--usb-notecard`, and optionally include the same GLW flags; their shapes (and
the route) count as occupied. Pass `--group a+b` (repeatable) to also report a
combined rectangle.

```shell
sl_map_cli --cache-dir cache \
  placement-slots \
  --usb-notecard usb_notecard.txt \
  --max-width 2048 --max-height 2048 \
  --group bottom_left+bottom_center
```

## Measuring a text label

The `measure-text` subcommand prints the pixel size a label would render at,
to help pick a font size or check whether it fits a slot. It produces no image.

```shell
sl_map_cli --font DejaVuSans.ttf \
  measure-text --font-px 28 --line "RRMC" --line "2026-04-18"
```
