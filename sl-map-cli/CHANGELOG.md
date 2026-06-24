# Changelog

## 0.6.0 - 2026-06-24 18:49:10Z

### 🚀 Features

- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes
- *(sl-types)* [**breaking**] Adopt value types migrated from sl-client
- *(sl-types)* [**breaking**] Widen GridRectangle to u32 and migrate more value
  types from sl-client

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Upgrade dependencies

## 0.5.0 - 2026-06-14 13:43:00Z

### 🚀 Features

- *(glw)* Add sl-glw crate for GlobalWind overlays
- *(cli)* Wire sl-glw overlay into sl-map-cli
- *(web)* Show embedded TTF name in font dropdown
- *(coverage)* Detect free placement slots before rendering
- Legend slot selection and arbitrary text labels
- *(web)* Add logo images to the library and map renders
- *(web)* Drive placement from the preview overlay with per-slot buttons
- *(text)* Align each line of a multi-line label individually
- *(web)* Optional per-region rectangle/name/coordinate overlays
- *(web)* Show region-name resolution progress in the render display
- *(web)* Preview missing-region and missing-tile fills
- *(sl-map-cli)* Reach render parity with sl-map-web

### 🐛 Bug Fixes

- *(map)* Handle degenerate USB notecard routes without divide-by-zero
- *(coverage)* Confine each placement slot to its own third of the map
- *(coverage)* Anchor a combined slot to the edges its group touches
- *(sl-map-apis)* Ship the duplicate-waypoint test fixture inside the crate

### 🚜 Refactor

- *(sl-map-apis)* De-duplicate shared packing and line-height logic
- *(sl-glw)* Drop dead error variants and make rendering infallible

### 📚 Documentation

- Fix rustdoc warnings for bare URL and private intra-doc links

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Bump sl-types, sl-map-apis dependencies in sl-glw, sl-map-cli
  and ls-map-web

## 0.4.0 - 2026-05-14 18:07:06Z

### 🚀 Features

- *(cli)* Add border options to from-usb-notecard for expanding the auto-derived
  rectangle
- *(apis)* Add progress event API for renders
- *(web)* Add sl-map-web crate with axum-based UI and JSON API
- *(web)* Add password authentication with LSL-driven registration
- *(cli)* Render indicatif progress bars from MapProgressEvent

### 🐛 Bug Fixes

- *(clippy)* Correct unreachable std::assert_matches path
- *(web)* Harden auth, CSRF, body limits, render caps, and CSP
- *(web)* Wrap config secrets in SecretString and zeroize signing key after use

### 🎨 Styling

- Apply pre-commit hook auto-formatting fixups

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.3.3 - 2026-04-08 11:33:57Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.3.2 - 2026-03-29 10:26:32Z

### 🚀 Features

- *(cli)* Add --metadata-output-file option that allows optionally writing the
  PPS and aspect ratio metadata to a file

### 🐛 Bug Fixes

- *(cliff)* Fix include_path -> include_paths

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies
- *(deny)* Remove unused OpenSSL license from deny.toml

## 0.3.1 - 2026-03-12 15:18:51Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.3.0 - 2026-02-15 19:33:32Z

### 💼 Other

- *(release)* Add release.sh and cliff config

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies

## 0.2.0

update dependencies

## 0.1.10

update dependencies

## 0.1.9

update dependencies

## 0.1.8

update dependencies

## 0.1.7

update dependencies

## 0.1.6

update dependencies

## 0.1.5

update dependencies

## 0.1.4

add arrows before waypoints
remove waypoint squares
make spline between waypoints dotted

## 0.1.3

replace simple lines between waypoints with Catmull-Rom spline
fix bug that could lead to an error if a waypoint was too close to 0
(closer than the size of the rectangle drawn at waypoints)

## 0.1.2

allow optionally filling in missing regions with a color
allow optionally filling in missing map tiles with a color
allow saving of the USB notecard map without a route to an extra file
before the route is drawn on it

## 0.1.1

Print PPS HUD config string and size recommendations when generating a map

Fix max zoom level calculations, previously it used the minimum zoom level out
of the one calculated for x and y axes but lower zoom levels are higher detail
so it needed to be the maximum.

## 0.1.0

Initial Release
