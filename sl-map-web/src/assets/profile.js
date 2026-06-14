// /profile and /profile/<uuid> page driver.

const UUID_RE_PROFILE =
  /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;

function $(id) {
  return document.getElementById(id);
}

function fmtDate(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  // ISO 8601 in local time, with a space instead of the "T" separator,
  // followed by the local UTC offset (and the timezone abbreviation).
  const pad = (n) => String(n).padStart(2, "0");
  // getTimezoneOffset() is minutes behind UTC, so a positive value means a
  // negative offset (e.g. -05:00). Invert it to render the ISO offset.
  const offMin = -d.getTimezoneOffset();
  const sign = offMin >= 0 ? "+" : "-";
  const absMin = Math.abs(offMin);
  const offset = `${sign}${pad(Math.floor(absMin / 60))}:${pad(absMin % 60)}`;
  let tzName = "";
  try {
    const parts = new Intl.DateTimeFormat(undefined, {
      timeZoneName: "short",
    }).formatToParts(d);
    const tz = parts.find((p) => p.type === "timeZoneName");
    if (tz) tzName = ` (${tz.value})`;
  } catch (_) {
    // Intl unavailable; fall back to just the numeric offset.
  }
  return (
    `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())} ` +
    `${pad(d.getHours())}:${pad(d.getMinutes())}:${pad(d.getSeconds())} ` +
    `${offset}${tzName}`
  );
}

// Pull the user id from the URL path. /profile → null (means "me");
// /profile/<uuid> → the uuid string.
function userIdFromPath() {
  const m = window.location.pathname.match(/^\/profile(?:\/([^/]+))?\/?$/);
  if (!m) return null;
  const raw = m[1];
  if (!raw) return null;
  return UUID_RE_PROFILE.test(raw) ? raw : "INVALID";
}

async function fetchJSON(url) {
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(await resp.text());
  return resp.json();
}

async function loadProfile() {
  const status = $("profile-status");
  let target = userIdFromPath();
  if (target === "INVALID") {
    status.textContent = "Invalid user id in URL.";
    return;
  }
  // If no id in the URL, fetch /api/auth/me first and use that.
  if (target === null) {
    try {
      const me = await fetchJSON("/api/auth/me");
      target = me.user_id;
    } catch (err) {
      status.textContent = `Failed to identify current user: ${err.message}`;
      return;
    }
  }
  let me = null;
  try {
    me = await fetchJSON("/api/auth/me");
  } catch (_err) {
    // ignore — viewing other users without me-context is still valid
  }
  let profile;
  try {
    profile = await fetchJSON(`/api/users/${encodeURIComponent(target)}`);
  } catch (err) {
    status.textContent = `Failed to load profile: ${err.message}`;
    return;
  }
  $("profile-legacy-name").textContent = profile.legacy_name;
  $("profile-username").textContent = profile.username;
  $("profile-uuid").textContent = profile.user_id;
  $("profile-created-at").textContent = fmtDate(profile.created_at);
  $("profile-fields").classList.remove("hidden");
  status.textContent = "";

  if (me && me.user_id === profile.user_id) {
    $("profile-self-actions").classList.remove("hidden");
    $("delete-account").addEventListener("click", () =>
      confirmDelete(profile.legacy_name),
    );
  }
}

async function confirmDelete(legacyName) {
  const ok = await confirmModal({
    title: "Delete account",
    message: `Delete the account for "${legacyName}"? This cannot be undone.`,
    danger: true,
    okText: "Delete account",
  });
  if (!ok) return;
  let resp;
  try {
    resp = await fetch("/api/users/me", { method: "DELETE" });
  } catch (err) {
    await alertModal({
      title: "Delete account",
      message: `Network error: ${err.message}`,
    });
    return;
  }
  if (!resp.ok) {
    await showError(resp, "Delete account");
    return;
  }
  await alertModal({
    title: "Account deleted",
    message:
      "Your account has been deleted. You will be returned to the login page.",
  });
  window.location.assign("/login");
}

document.addEventListener("DOMContentLoaded", loadProfile);
