# Changelog

## 0.8.0 - 2026-09-17 22:04:11Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update rustls and chacha20 to clear cargo-deny
- *(dependencies)* Update dependencies

## 0.7.0 - 2026-08-26 13:49:58Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.6.0 - 2026-07-23 11:24:16Z

### 🚀 Features

- *(sl-types)* Derive Ord/PartialOrd on key types
- *(sl-types)* Add serde Serialize/Deserialize across public types

### 🐛 Bug Fixes

- *(sl-map-apis)* Migrate font name lookup off unmaintained ttf-parser
- *(sl-types)* Percent-encode spaces in Location::as_maps_url

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Update dependencies

## 0.5.0 - 2026-07-23 11:17:51Z

### 🚀 Features

- *(sl-types)* Derive Ord/PartialOrd on key types
- *(sl-types)* Add serde Serialize/Deserialize across public types

### 🐛 Bug Fixes

- *(sl-map-apis)* Migrate font name lookup off unmaintained ttf-parser
- *(sl-types)* Percent-encode spaces in Location::as_maps_url

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Update dependencies

## 0.4.0 - 2026-06-24 18:49:09Z

### 🚀 Features

- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes
- *(sl-types)* [**breaking**] Adopt value types migrated from sl-client
- *(sl-types)* [**breaking**] Widen GridRectangle to u32 and migrate more value
  types from sl-client

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Upgrade dependencies

## 0.3.1 - 2026-06-14 13:15:46Z

### 🚀 Features

- *(glw)* Add sl-glw crate for GlobalWind overlays
- *(web)* Show embedded TTF name in font dropdown
- Legend slot selection and arbitrary text labels
- *(sl-map-cli)* Reach render parity with sl-map-web

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version

## 0.3.0 - 2026-05-14 18:07:06Z

### 🚀 Features

- *(cli)* Add border options to from-usb-notecard for expanding the auto-derived
  rectangle
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

## 0.2.3 - 2026-04-08 11:33:57Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.2.2 - 2026-03-29 10:26:31Z

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

## 0.2.0 - 2026-02-15 19:33:31Z

### 🚀 Features

- *(chatlog)* Parse message about a parsing being unavailable for voice

### 💼 Other

- *(release)* Add release.sh and cliff config

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Upgrade dependencies
- *(chumsky)* Update chumsky dependency from 0.9.3 to 0.12.0

## 0.1.5

update dependencies

## 0.1.4

fix parsing of entered/left region lines

## 0.1.3

update dependencies

## 0.1.2

update dependencies

## 0.1.1

remove Box suggested by clippy to make pattern matching easier

## 0.1.0

Initial Release
