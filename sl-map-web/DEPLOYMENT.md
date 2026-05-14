# Deploying sl-map-web

This is a single-host operations runbook for `sl-map-web` running as a
systemd service behind an unspecified TLS-terminating reverse proxy with an
unspecified ACME (HTTP-01) client. Tool choices are intentionally left to
the operator; the document only states the interface contracts that any
choice has to meet.

The example hostname throughout this document is `maps.example.org` — swap
in whatever public hostname you actually use.

Required environment variables and the application-level authentication
flow live in [`README.md`](README.md); this document is the deployment
runbook, not the configuration reference.

## 1. Overview & assumptions

- One Linux host with systemd.
- Dedicated unprivileged system user `sl-map-web` runs the service.
- `sl_map_web` binds loopback only (default `127.0.0.1:3000`); the reverse
  proxy terminates TLS and forwards to it.
- A separate ACME client obtains and renews the certificate via the HTTP-01
  challenge.
- SQLite is the only datastore; migrations are embedded and applied on
  startup.
- Static assets are embedded in the binary — no `static/` directory needs
  to be deployed alongside it.

## 2. Pre-flight checklist

- `A` and `AAAA` records for the chosen hostname point at the server's
  public addresses.
- TCP/80 on the host is reachable from the public internet (HTTP-01).
- TCP/443 on the host is reachable from the public internet.
- The reverse proxy and the ACME client are already installed and running
  for at least one other site (this guide adds one more vhost — it does not
  cover bootstrapping the proxy or ACME client from scratch).
- The server's glibc is `>=` the build host's glibc (`ldd --version` on
  both). If they don't line up, build on the server or use a musl target.

## 3. Build

From a checkout of this repository:

```sh
cargo build --release -p sl-map-web --bin sl_map_web
```

Output: `target/release/sl_map_web`. Single self-contained binary; static
HTML/CSS/JS and SQLite migrations are baked in.

## 4. System user & directories

```sh
useradd --system --home /var/lib/sl-map-web --shell /usr/sbin/nologin sl-map-web
install -d -o root -g sl-map-web -m 0750 /etc/sl-map-web
```

`/var/lib/sl-map-web/` is created automatically by `StateDirectory=` in
the unit file on first start, owned by `sl-map-web:sl-map-web`, mode `0750`.

## 5. Install the binary

From the build host:

```sh
scp target/release/sl_map_web root@maps.example.org:/usr/local/bin/sl_map_web.new
```

On the server:

```sh
install -o root -g root -m 0755 /usr/local/bin/sl_map_web.new /usr/local/bin/sl_map_web
rm /usr/local/bin/sl_map_web.new
```

The `install` step is an atomic rename, so a concurrent `systemctl restart`
won't see a partially-written file.

## 6. Generate secrets and write the env file

Generate both secrets **on the server** so they never live on a workstation:

```sh
openssl rand -base64 64 | tr -d '\n' # SL_MAP_WEB_SESSION_SIGNING_KEY
openssl rand -hex 32                 # SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN
```

Write `/etc/sl-map-web/sl-map-web.env`, mode `0640`, owner
`root:sl-map-web`. Template:

```ini
SL_MAP_WEB_BIND=127.0.0.1:3000
SL_MAP_WEB_PUBLIC_BASE_URL=https://maps.example.org
SL_MAP_WEB_CACHE_DIR=/var/lib/sl-map-web/cache
SL_MAP_WEB_STORAGE_DIR=/var/lib/sl-map-web/storage
SL_MAP_WEB_DATABASE_URL=sqlite:///var/lib/sl-map-web/sl-map-web.db?mode=rwc
SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN=<from openssl rand -hex 32>
SL_MAP_WEB_SESSION_SIGNING_KEY=<from openssl rand -base64 64>
SL_MAP_WEB_TRUSTED_PROXIES=127.0.0.1/32,::1/128
SL_MAP_WEB_FORWARDED_HEADER=x-forwarded-for
RUST_LOG=info,sl_map_web=info
# BACKUP_DIR is consumed by sl-map-web-backup.service (see §12).
BACKUP_DIR=/var/backups/sl-map-web
```

Then:

```sh
chown root:sl-map-web /etc/sl-map-web/sl-map-web.env
chmod 0640 /etc/sl-map-web/sl-map-web.env
```

`SL_MAP_WEB_TRUSTED_PROXIES` must list every reverse proxy whose
`X-Forwarded-For` header the application should trust. **Every listed
proxy MUST replace, not append to, that header** — otherwise an attacker
can spoof the client IP. See the security contract in
[`src/config.rs`](src/config.rs) for the full discussion.

## 7. Install the systemd unit

```sh
install -o root -g root -m 0644 contrib/sl-map-web.service \
  /etc/systemd/system/sl-map-web.service
systemctl daemon-reload
systemctl enable --now sl-map-web
journalctl -fu sl-map-web
```

Expect a log line like `starting sl-map-web bind=127.0.0.1:3000` followed
by `recovered 0 orphaned in_progress render(s) at startup`.

## 8. Reverse-proxy requirements (tool-agnostic)

The reverse proxy must:

- Terminate TLS for the chosen hostname.
- Forward to `127.0.0.1:3000` (or whatever `SL_MAP_WEB_BIND` is set to).
  HTTP/1.1 to the backend is sufficient.
- **Replace** `X-Forwarded-For` with the immediate client IP. Do not
  pass the client-supplied value through; doing so breaks the trust model
  documented in [`src/config.rs`](src/config.rs).
- Forward the `Host` header unchanged.

## 9. Certificate / ACME requirements (tool-agnostic)

The ACME client must satisfy the HTTP-01 challenge at:

```text
http://maps.example.org/.well-known/acme-challenge/<token>
```

Two common patterns work; either is fine:

- **Proxy through the reverse proxy.** The reverse proxy listens on port
  80 and routes `/.well-known/acme-challenge/` to whatever responder the
  ACME client provides.
- **Dedicated static HTTP server.** A small static-file server listens on
  port 80 and serves a webroot the ACME client writes into.

### Webroot filesystem permissions

When the ACME client writes challenge files into a webroot served by a
separate static HTTP server, get the permissions right or renewals will
silently fail:

- Webroot tree: directories `0755`, files `0644` (world-readable).
- `<webroot>/.well-known/` and `<webroot>/.well-known/acme-challenge/`
  exist and are owned by the ACME client's user, with the static
  server's group, mode `2775` (setgid). The setgid bit ensures challenge
  files the ACME client drops in inherit the static server's group, so
  the static server can read them even if the ACME client's umask is
  restrictive.
- The certificate file the reverse proxy reads must be readable by the
  proxy's user. Coordinate ownership/group between the ACME client
  (writer) and the proxy (reader), and configure the ACME client to
  signal the proxy (reload / restart) after each renewal.

### Trusted-proxy CIDR

Once the proxy is in place, the env file's `SL_MAP_WEB_TRUSTED_PROXIES`
must cover the proxy's source address as it appears to `sl-map-web`. For
a loopback proxy on the same host, `127.0.0.1/32,::1/128` is correct. For
a remote proxy, use its actual address — never a wider range than
necessary.

## 10. First-run verification

```sh
curl -fsS -o /dev/null -w '%{http_code}\n' https://maps.example.org/login
# expect: 200

curl -vI https://maps.example.org/ 2>&1 |
  grep -E '^\* (Server certificate|subject|issuer|expire)'
# inspect the chain

journalctl -u sl-map-web --since '5 minutes ago'
# no panics, "recovered N orphaned in_progress render(s)" present
```

Register a test account end-to-end using the helper binary shipped in
this repo (it emulates the in-world LSL registration object):

```sh
cargo run --bin sl_map_web_register -- --help
# fill in --server-url, --bearer-token, --agent-key, --legacy-name, --username
```

The command prints a `set_password_url`; opening it in a browser lets you
choose a password and gives you a working login.

## 11. Updates / redeploy

Build a new `sl_map_web` on the build host, then on the server:

```sh
# stash the currently-running binary for one-step rollback
cp /usr/local/bin/sl_map_web /usr/local/bin/sl_map_web.prev

# atomic install of the new binary
scp target/release/sl_map_web root@maps.example.org:/usr/local/bin/sl_map_web.new
ssh root@maps.example.org install -o root -g root -m 0755 \
  /usr/local/bin/sl_map_web.new /usr/local/bin/sl_map_web

systemctl restart sl-map-web
```

Tail the journal and re-run the verification curls from §10.

Rollback:

```sh
install -o root -g root -m 0755 /usr/local/bin/sl_map_web.prev /usr/local/bin/sl_map_web
systemctl restart sl-map-web
```

Migrations are embedded and applied on startup
([`migrations/`](migrations/)). A failed migration aborts startup, so a
bad release fails fast rather than corrupting state — but schema changes
are forward-only between builds. Rolling back a release that introduced
a migration requires also restoring the SQLite DB from the latest backup
(§12).

## 12. Backups

### SQLite database

Do **not** copy `sl-map-web.db` while the service is running — the WAL
means a plain `cp` can produce an inconsistent snapshot. Use SQLite's
online backup API. Ready-to-install units live in [`contrib/`](contrib/):

- `sl-map-web-backup.service` — oneshot. Backs up via
  `sqlite3 ".backup …"` to `${BACKUP_DIR}/sl-map-web.db.new`, then
  atomically renames over `${BACKUP_DIR}/sl-map-web.db`. There is only
  ever one snapshot at rest; a failed run leaves the previous good
  snapshot in place.
- `sl-map-web-backup.timer` — daily, randomised within a 30-minute
  window, `Persistent=true` so it catches up after host downtime.

Install:

```sh
install -o root -g root -m 0644 contrib/sl-map-web-backup.service \
  /etc/systemd/system/sl-map-web-backup.service
install -o root -g root -m 0644 contrib/sl-map-web-backup.timer \
  /etc/systemd/system/sl-map-web-backup.timer
install -d -o sl-map-web -g sl-map-web -m 0750 /var/backups/sl-map-web
# Make sure BACKUP_DIR=/var/backups/sl-map-web is set in
# /etc/sl-map-web/sl-map-web.env (see §6).
systemctl daemon-reload
systemctl enable --now sl-map-web-backup.timer
systemctl start sl-map-web-backup.service # smoke-test one run
ls -la /var/backups/sl-map-web/           # expect exactly one .db file
```

If `BACKUP_DIR` lives outside `/var/lib/sl-map-web` (as in the example
above, where it lives under `/var/backups/`), the backup unit's
`ReadWritePaths=` has to be extended to cover it:

```sh
systemctl edit sl-map-web-backup.service
# [Service]
# ReadWritePaths=/var/backups/sl-map-web
```

Long-term retention is the responsibility of the operator's existing
file-level backup system (borg, restic, snapshots, off-host rsync, …).
The timer always overwrites the same file, so every off-host backup run
sees a fresh, consistent snapshot and keeps whatever history its policy
dictates.

### Other state

- `/var/lib/sl-map-web/storage/renders/` — saved render images. Include
  in the file-level backup. Loss is recoverable from re-render but means
  user-visible breakage of saved-render links.
- `/var/lib/sl-map-web/cache/` — tile / region cache. **Exclude from
  backups.** It re-fills on demand.

### Restore

1. `systemctl stop sl-map-web sl-map-web-backup.timer`.
2. Restore the SQLite snapshot to `/var/lib/sl-map-web/sl-map-web.db`.
3. Delete any stale `sl-map-web.db-wal` / `sl-map-web.db-shm` next to it
   — these are tied to the previous WAL state and will confuse SQLite if
   left behind.
4. Restore `storage/` from the file-level backup.
5. `systemctl start sl-map-web sl-map-web-backup.timer`.
6. Re-run the verification curls from §10.

## 13. Log inspection

The service logs to stdout, captured by journald.

```sh
journalctl -u sl-map-web -f                              # live tail
journalctl -u sl-map-web --since today --grep ERROR      # today's errors
journalctl -u sl-map-web --since '1 hour ago' --no-pager # recent
```

To temporarily raise log verbosity without editing the env file:

```sh
systemctl edit sl-map-web
# [Service]
# Environment=RUST_LOG=debug,sl_map_web=trace
systemctl restart sl-map-web
# investigate, then `systemctl revert sl-map-web` to drop the override.
```

## 14. Secret rotation

All three rotations are: edit `/etc/sl-map-web/sl-map-web.env`, then
`systemctl restart sl-map-web`. The user-visible side effects differ:

- **`SL_MAP_WEB_SESSION_SIGNING_KEY`** — every existing session is
  invalidated; every user has to log in again. Plan this for a low-
  traffic window or announce it.
- **`SL_MAP_WEB_LSL_REGISTRATION_BEARER_TOKEN`** — the in-world LSL
  registration object stops working until the new token is loaded into
  it. Coordinate with whoever owns the script before rotating.
- **`SL_MAP_WEB_COOKIE_NAME`** — changing this also forces every user to
  log in again (the browser ignores the cookie under the old name).
  Useful as an emergency "log everyone out now" lever.
