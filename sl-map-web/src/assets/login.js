const form = document.getElementById("login-form");
const statusEl = document.getElementById("login-status");
const params = new URLSearchParams(window.location.search);
// Validate `next` so an attacker-crafted `/login?next=https://evil/`
// cannot bounce the user off-site after a real sign-in. Only
// same-origin URLs are honoured; anything else falls back to `/`.
const next = (() => {
  const raw = params.get("next");
  if (!raw) return "/";
  try {
    const url = new URL(raw, window.location.origin);
    if (url.origin !== window.location.origin) return "/";
    return url.pathname + url.search + url.hash;
  } catch {
    return "/";
  }
})();

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  statusEl.textContent = "Signing in…";
  try {
    const resp = await fetch("/api/auth/login", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        identifier: document.getElementById("identifier").value,
        password: document.getElementById("password").value,
      }),
    });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      throw new Error(data.error || `HTTP ${resp.status}`);
    }
    window.location.assign(next);
  } catch (err) {
    statusEl.textContent = `Sign-in failed: ${err.message}`;
  }
});
