# Changelog

## 0.4.0 - 2026-06-22 11:59:34Z

### 🚀 Features

- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version

## 0.3.2 - 2026-06-22 11:55:02Z

### 🚀 Features

- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version

## 0.3.1 - 2026-06-14 13:09:49Z

### 🚀 Features

- *(glw)* Add sl-glw crate for GlobalWind overlays
- *(web)* Show embedded TTF name in font dropdown
- Legend slot selection and arbitrary text labels
- *(sl-map-cli)* Reach render parity with sl-map-web

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version

## 0.3.0 - 2026-05-14 18:07:07Z

### 🚀 Features

- *(cli)* Add border options to from-usb-notecard for expanding the auto-derived
  rectangle
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

## 0.2.3 - 2026-04-08 11:33:58Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.2.2 - 2026-03-29 10:26:32Z

### 🚀 Features

- *(cli)* Add --metadata-output-file option that allows optionally writing the
  PPS and aspect ratio metadata to a file

### 🐛 Bug Fixes

- *(cliff)* Fix include_path -> include_paths

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies
- *(deny)* Remove unused OpenSSL license from deny.toml

## 0.2.1 - 2026-03-12 15:18:51Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.2.0 - 2026-02-15 19:33:32Z

### 💼 Other

- *(release)* Add release.sh and cliff config

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies
- *(chumsky)* Update chumsky dependency from 0.9.3 to 0.12.0

## 0.1.10

update dependencies

## 0.1.9

fix parsing of area for the region case

## 0.1.8

update dependencies

## 0.1.7

update dependencies

## 0.1.6

update dependencies
add a lot of new types for other areas of SL related types besides maps

## 0.1.5

update dependencies

## 0.1.4

add money module with LindenAmount type

## 0.1.3

update dependencies

## 0.1.2

update dependencies

## 0.1.1

add function to `GridRectangleLike` that returns the PPS HUD config string
for a given grid rectangle

fix max zoom level calculations

## 0.1.0

Initial Release
