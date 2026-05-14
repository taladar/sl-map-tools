# Changelog

## 0.2.1 - 2026-05-14 18:31:23Z

### 🐛 Bug Fixes

- *(release)* Set explicit archive name for multi-binary web release
- *(sl_map_web)* Fix cliff.toml to include the github action path

## 0.2.0 - 2026-05-14 18:07:07Z

### 🚀 Features

- *(cli)* Add --metadata-output-file option that allows optionally writing the
  PPS and aspect ratio metadata to a file
- *(cli)* Add border options to from-usb-notecard for expanding the auto-derived
  rectangle
- *(apis)* Add progress event API for renders
- *(web)* Add sl-map-web crate with axum-based UI and JSON API
- *(web)* Add password authentication with LSL-driven registration
- *(web)* Add groups and shared notecard/render library
- *(cli)* Render indicatif progress bars from MapProgressEvent
- *(web)* Library detail columns, metadata modal, notecard source subtabs
- *(web)* Name saved-render downloads after notecard or grid corners
- *(web)* Image-viewer modal with with-route / without-route comparison
- *(web)* User profile pages, account self-deletion, deleted-user fallbacks
- *(web)* Add a legend for the tile-status colors on the render progress grid
- *(web)* Add map.sh's green to the route colour presets
- *(web)* Persist the user's route colour on their account row

### 🐛 Bug Fixes

- *(area)* Fix region parsing
- *(cliff)* Fix include_path -> include_paths
- *(web)* Harden auth, CSRF, body limits, render caps, and CSP
- *(web)* Close TOCTOU races in group role/membership and invitation flips
- *(web)* Store SHA-256 of session id at rest
- *(web)* Gate live render artifacts and collapse existence leaks to 404
- *(web)* Gate SSE render-events stream behind the read-render check
- *(web)* Fold group-membership check into library listing SQL
- *(web)* Return generic body for 500-class errors
- *(web)* Round-trip render metadata and settings JSON through typed structs
- *(web)* Validate group and notecard display names
- *(web)* Reject unicode Cf format characters in display names
- *(web)* Replace native dialogs with in-page modals
- *(web)* Add per-user token-bucket rate limit on creates
- *(web)* Validate UUID query params before using them in fetch / form
- *(web)* Equalise LslBearer compare timing across length mismatch
- *(web)* Throttle sessions.last_seen_at bump to once per 60 s
- *(web)* Reject cookie_secure / public_base_url scheme mismatch at startup
- *(web)* Wrap config secrets in SecretString and zeroize signing key after use
- *(web)* Sanitise error display before logging and demote 4xx to debug
- *(web)* Raise password minimum to 12 and add 128-byte maximum
- *(web)* Prune outstanding set-password tokens on re-register
- *(web)* Canonicalise persisted render colours to #rrggbb
- *(web)* Accept "jpg" as an alias for "jpeg" on the render JSON API
- *(web)* Enforce notecard / render scope match at the DB and copy on reuse
- *(web)* Recover orphaned in_progress renders at startup
- *(web)* Rotate the cookie-carried session on login
- *(web)* Escape and demote forwarded-header parse-warn logs
- *(web)* Alias renamed columns in the saved-renders list queries
- *(web)* Accept notecard_id in the derive-rectangle preview endpoint
- *(web)* Run migrations with PRAGMA foreign_keys=OFF on a dedicated pool

### 💼 Other

- *(release)* Add release.sh and cliff config

### 📚 Documentation

- *(web)* Document trusted_proxies security contract and analysis scope
- *(web)* Document file log sink as debug-only, not for production
- *(web)* Codify the BREACH contract at the compression layer
- *(web)* Expand stubby response-wrapper field docs
- *(web)* Add deployment runbook and sample systemd units

### 🎨 Styling

- Apply pre-commit hook auto-formatting fixups

### ⚙️ Miscellaneous Tasks

- *(dependencies)* Update dependencies and fix lints
- *(dependencies)* Update dependencies
- *(dependencies)* Upgrade dependencies
- *(chumsky)* Update chumsky dependency from 0.9.3 to 0.12.0
- *(release)* Release new version
- *(dependencies)* Update dependencies
- *(release)* Release new version
- *(dependencies)* Upgrade dependencies
- *(deny)* Remove unused OpenSSL license from deny.toml
- *(release)* Release new version
- *(dependencies)* Update dependencies
- *(release)* Release new version
- *(dependencies)* Update dependencies
- *(web)* Add packaging files for binary-crate release flow

## 0.1.0 - Unreleased

### 🚀 Features

- Initial release of `sl-map-web`: axum-based HTTP service exposing the
  same render capabilities as `sl-map-cli` (from-grid-rectangle and
  from-usb-notecard) plus an embedded HTML/JS frontend with client-side
  preview, real-time Server-Sent Events progress, and a JSON / multipart
  API for programmatic clients.
