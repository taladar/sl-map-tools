# sl-map-web

Web UI and JSON API for the SL map renderer. Same capabilities as
`sl-map-cli` (render from a grid rectangle, render from a USB notecard with
route overlay), exposed over HTTP via axum.

## Authentication

All UI and render endpoints require a logged-in session. The expected flow is:

1. The avatar clicks an in-world LSL object.
2. The object calls `POST /api/auth/register` with `Authorization: Bearer
   <pre-shared token>` and a JSON body containing the avatar's UUID
   (`agent_key`), `legacy_name` (e.g. `"Foo Resident"`), and `username`
   (e.g. `"foo.resident"`).
3. The server upserts the user (UUID is the stable primary key; legacy name
   and username are refreshed on every call), generates a single-use
   time-limited token, and returns
   `{ set_password_url, expires_at, user }`.
4. The LSL script chats the `set_password_url` back to the avatar.
5. The avatar opens the URL, picks a password, and the token is consumed.
6. Subsequent visits sign in at `/login` with the chosen password against
   either the UUID, the `firstname.lastname` username, or the legacy
   `Firstname Lastname` display.

Running a password-set flow on an existing account doubles as a password
reset — all of that user's existing sessions are revoked when a new password
is set.

## Required configuration

These environment variables (or matching CLI flags) **must** be set; the
binary refuses to start if any of them is missing or invalid.

| Variable | Purpose |
|---|---|
| `SL_MAP_WEB_CACHE_DIR` | tile / region cache directory shared with the CLI |
| `SL_MAP_WEB_STORAGE_DIR` | directory for saved render images and uploaded logos (subdirectories are created on startup) |
| `SL_MAP_WEB_FONTS_DIR` | directory of selectable `.ttf` fonts for text overlays (GLW labels/legend, region names, text labels); must exist and contain at least one `.ttf` at startup (the workspace ships `DejaVuSans.ttf`) |
| `SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN` | pre-shared secret presented by the LSL registration object |
| `SL_MAP_WEB_SESSION_SIGNING_KEY` | base64-encoded ≥ 64 bytes used to sign the session cookie |
| `SL_MAP_WEB_PUBLIC_BASE_URL` | external base URL (e.g. `https://maps.example.org`) used to build set-password links |

The remaining settings have sensible defaults — see `--help` for the full
list, including the trusted-proxy / forwarded-header settings (`SL_MAP_WEB_TRUSTED_PROXIES`,
`SL_MAP_WEB_FORWARDED_HEADER`) used when the server runs behind a reverse
proxy.

The service does not terminate TLS. Run it behind a reverse proxy (nginx,
Caddy, Traefik, …) that terminates HTTPS — the default `cookie_secure=true`
means the session cookie will only be sent over HTTPS connections.
