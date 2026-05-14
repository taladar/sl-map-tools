# Changelog

## 0.4.0 - 2026-05-14 18:07:06Z

### 🚀 Features

- *(cli)* Add border options to from-usb-notecard for expanding the auto-derived
  rectangle
- *(apis)* Add progress event API for renders
- *(web)* Add sl-map-web crate with axum-based UI and JSON API
- *(web)* Add password authentication with LSL-driven registration
- *(cli)* Render indicatif progress bars from MapProgressEvent

### 🐛 Bug Fixes

- *(web)* Harden auth, CSRF, body limits, render caps, and CSP
- *(web)* Wrap config secrets in SecretString and zeroize signing key after use

### 🎨 Styling

- Apply pre-commit hook auto-formatting fixups

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.3.3 - 2026-04-08 11:33:57Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.3.2 - 2026-03-29 10:26:31Z

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

## 0.3.0 - 2026-02-15 19:33:31Z

### 💼 Other

- *(release)* Add release.sh and cliff config

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies

## 0.2.0

update dependencies
new error variant from uniform_cubic_splice calculations

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

add methods to the `MapTileCache` to check if a map tile or a region exist
add the option to fill missing map tiles in `Map` with a color
add the option to fill missing regions in lower detail map tiles with a color

## 0.1.1

update sl-types dependency to 0.1.1

## 0.1.0

Initial Release
