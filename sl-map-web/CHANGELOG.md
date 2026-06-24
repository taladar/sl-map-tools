# Changelog

## 0.4.0 - 2026-06-24 18:49:10Z

### 🚀 Features

- *(sl-map-web)* Add a shared per-user saved-colour palette
- *(sl-map-web)* Add save/load themes for render presentation settings
- *(sl-types)* Copy/Hash + uuid()/From<Uuid> on all key newtypes
- *(sl-types)* [**breaking**] Adopt value types migrated from sl-client
- *(sl-types)* [**breaking**] Widen GridRectangle to u32 and migrate more value
  types from sl-client

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Upgrade dependencies

## 0.3.0 - 2026-06-14 13:43:01Z

### 🚀 Features

- *(glw)* Add sl-glw crate for GlobalWind overlays
- *(web)* Wire sl-glw overlay into sl-map-web
- *(web)* View and download saved GLW JSON in library
- *(web)* Show embedded TTF name in font dropdown
- *(web)* Use tabs for GLW source selection
- *(web)* Prefill GLW style swatches with real render defaults
- *(coverage)* Detect free placement slots before rendering
- Legend slot selection and arbitrary text labels
- *(web)* Add logo images to the library and map renders
- *(web)* Mark final-image bounds in the tile preview
- *(web)* Show the GLW overlay in the tile preview
- *(web)* Exclude the legend from the preview GLW overlay and test it
- *(web)* Show free placement slots as a toggleable preview overlay
- *(web)* Show the GLW legend in the preview at its chosen slot
- *(web)* Show labels & logos in the preview; fix render placement
- *(web)* Move Preview button to the Preview panel and Generate to Render
- *(web)* Drive placement from the preview overlay with per-slot buttons
- *(web)* Add a 4x logo scale option
- *(text)* Align each line of a multi-line label individually
- *(web)* Rework the GLW overlay panel
- *(web)* Show the full slot label on hover when it is clipped
- *(web)* Render the preview route identically to the final output
- *(web)* Allow uploading a logo from the Add-logo modal
- *(web)* Split the library page into tabs with working cross-links
- *(web)* Split the Add-logo modal into reuse / upload subtabs
- *(web)* Edge extend/shrink buttons and drag-select zoom on the preview
- *(web)* Optional per-region rectangle/name/coordinate overlays
- *(web)* Show selected region count on the preview
- *(web)* Show region-name resolution progress in the render display
- *(web)* Stream region-name progress during the overlay preview
- *(web)* Preview missing-region and missing-tile fills
- *(web)* Add Metadata button to the finished-render view
- *(sl-map-cli)* Reach render parity with sl-map-web
- *(web)* Show profile dates in ISO 8601 with timezone
- *(web)* Add region-name search to grid rectangle mode

### 🐛 Bug Fixes

- *(map)* Handle degenerate USB notecard routes without divide-by-zero
- *(coverage)* Confine each placement slot to its own third of the map
- *(web)* Allow blob: images in CSP so the GLW preview overlay loads
- *(web)* Only offer to combine slots whose rectangles actually touch
- *(web)* Only allow slot combinations that form a solid rectangle
- *(web)* Don't let one overflowing placement blank the whole preview
- *(web)* Clear stale overlay error when the preview auto-refreshes
- *(coverage)* Anchor a combined slot to the edges its group touches
- *(render)* Reject over-full placements before saving the render
- *(web)* Hide the GLW legend slot button when GLW is disabled
- *(web)* Decode region-overlay preview PNG without fetch (CSP)
- *(web)* Authorize logo reads in the placement-preview endpoints
- *(web)* Bound render/preview output by area, not just per-side dimension
- *(web)* Fail the render row if logo linking fails after insert
- *(web)* Enforce same-library scope for a render's saved GLW data
- *(web)* Don't run render-form wiring on pages without the render form
- *(web)* Revoke the logo-upload preview object URL when the modal closes
- *(web)* Use the in-page promptModal for GLW/logo rename
- *(web)* Swap default fill colors for missing map tiles vs regions
- *(sl-map-apis)* Ship the duplicate-waypoint test fixture inside the crate
- *(web)* Bound the glw_preview output image by area and dimension
- *(web)* Add missing Profile link to the invitations page nav
- *(web)* Avoid rustdoc private intra-doc link warning in glw_preview

### 🚜 Refactor

- *(sl-map-apis)* De-duplicate shared packing and line-height logic
- *(sl-glw)* Drop dead error variants and make rendering infallible

### 📚 Documentation

- Fix rustdoc warnings for bare URL and private intra-doc links
- *(web)* Explain that render-created GLW rows are not orphans
- *(web)* Correct LogoPlacement::scale allowed values (1, 2 or 4)

### ⚙️ Miscellaneous Tasks

- *(release)* Release new version
- *(release)* Release new version
- *(release)* Release new version
- *(dependencies)* Bump sl-types, sl-map-apis dependencies in sl-glw, sl-map-cli
  and ls-map-web

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
