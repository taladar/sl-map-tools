# Changelog

## 0.6.0 - 2026-09-17 22:04:11Z

### 🚀 Features

- *(sl-map-apis)* [**breaking**] Configurable route style with per-section
  overrides

### 🐛 Bug Fixes

- *(sl-map-apis)* Put route arrowheads back on the waypoints

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update rustls and chacha20 to clear cargo-deny
- *(dependencies)* Update dependencies

## 0.5.0 - 2026-08-26 13:49:58Z

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies

## 0.4.0 - 2026-07-23 11:24:17Z

### 🚀 Features

- *(sl-types)* Derive Ord/PartialOrd on key types
- *(sl-types)* Add serde Serialize/Deserialize across public types
- *(sl-map-apis)* Configurable map tile base URL

### 🐛 Bug Fixes

- *(sl-map-apis)* Migrate font name lookup off unmaintained ttf-parser
- *(sl-types)* Percent-encode spaces in Location::as_maps_url

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Update dependencies

## 0.3.0 - 2026-07-23 11:17:52Z

### 🚀 Features

- *(sl-types)* Derive Ord/PartialOrd on key types
- *(sl-types)* Add serde Serialize/Deserialize across public types
- *(sl-map-apis)* Configurable map tile base URL

### 🐛 Bug Fixes

- *(sl-map-apis)* Migrate font name lookup off unmaintained ttf-parser
- *(sl-types)* Percent-encode spaces in Location::as_maps_url

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Update dependencies

## 0.2.0 - 2026-06-24 18:49:09Z

### 🚀 Features

- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes
- *(sl-types)* [**breaking**] Adopt value types migrated from sl-client
- *(sl-types)* [**breaking**] Widen GridRectangle to u32 and migrate more value
  types from sl-client

### 🐛 Bug Fixes

- *(cliff)* Fix include_paths in sl-glw
- *(cliff)* Fix sl-glw tag prefix to match hyphenated tags

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Upgrade dependencies

## 0.1.0

Initial Release
