const form = document.getElementById("setpw-form");
const statusEl = document.getElementById("setpw-status");
const params = new URLSearchParams(window.location.search);
const token = params.get("token") || "";

if (!token) {
  statusEl.textContent =
    "Missing token. Click the in-world object again to receive a fresh link.";
  form.querySelector("button").disabled = true;
}

form.addEventListener("submit", async (e) => {
  e.preventDefault();
  const password = document.getElementById("password").value;
  const confirm = document.getElementById("confirm").value;
  if (password !== confirm) {
    statusEl.textContent = "Passwords do not match.";
    return;
  }
  statusEl.textContent = "Saving…";
  try {
    const resp = await fetch("/api/auth/set-password", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ token, new_password: password }),
    });
    if (!resp.ok) {
      const data = await resp.json().catch(() => ({}));
      throw new Error(data.error || `HTTP ${resp.status}`);
    }
    statusEl.textContent = "Password set — redirecting to sign in…";
    setTimeout(() => window.location.assign("/login"), 1000);
  } catch (err) {
    statusEl.textContent = `Could not set password: ${err.message}`;
  }
});
