// /library page driver.

const SCOPE_KEY = "sl-map-web.library.scope";

function $(id) {
  return document.getElementById(id);
}

function fmtDate(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function statusBadge(status, errorMessage) {
  const span = document.createElement("span");
  span.className = `status-pill status-${status}`;
  if (status === "failed" && errorMessage) {
    span.title = errorMessage;
    span.textContent = "failed";
  } else if (status === "in_progress") {
    span.textContent = "in progress";
  } else {
    span.textContent = status;
  }
  return span;
}

async function fetchJSON(url) {
  const resp = await fetch(url);
  if (!resp.ok) throw new Error(await resp.text());
  return resp.json();
}

async function populateScopes() {
  const sel = $("scope-select");
  const groups = await fetchJSON("/api/groups");
  const options = [["personal", "Personal"]];
  for (const g of groups.groups || []) {
    options.push([`group:${g.group_id}`, `Group: ${g.name} (${g.my_role})`]);
  }
  sel.replaceChildren();
  for (const [value, label] of options) {
    const opt = document.createElement("option");
    opt.value = value;
    opt.textContent = label;
    sel.appendChild(opt);
  }
  const saved = sessionStorage.getItem(SCOPE_KEY);
  if (saved && options.some(([v]) => v === saved)) {
    sel.value = saved;
  }
  sel.addEventListener("change", () => {
    sessionStorage.setItem(SCOPE_KEY, sel.value);
    refresh();
  });
}

function notecardRow(n) {
  const tr = document.createElement("tr");
  tr.appendChild(td(n.name));
  tr.appendChild(td(n.uploaded_by_legacy_name));
  tr.appendChild(td(fmtDate(n.created_at)));
  const actions = document.createElement("td");
  const dl = document.createElement("a");
  dl.href = `/api/notecards/${n.notecard_id}/text`;
  dl.textContent = "Download";
  dl.className = "row-action";
  actions.appendChild(dl);
  const reuse = document.createElement("a");
  reuse.href = `/?reuse_notecard=${n.notecard_id}`;
  reuse.textContent = "Render";
  reuse.className = "row-action";
  actions.appendChild(reuse);
  const del = document.createElement("button");
  del.type = "button";
  del.textContent = "Delete";
  del.className = "row-action danger";
  del.addEventListener("click", async () => {
    if (!confirm(`Delete notecard "${n.name}"?`)) return;
    const resp = await fetch(`/api/notecards/${n.notecard_id}`, {
      method: "DELETE",
    });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    refresh();
  });
  actions.appendChild(del);
  tr.appendChild(actions);
  return tr;
}

function renderRow(r) {
  const tr = document.createElement("tr");
  tr.dataset.renderId = r.render_id;
  const statusTd = document.createElement("td");
  statusTd.appendChild(statusBadge(r.status, r.error_message));
  tr.appendChild(statusTd);
  tr.appendChild(td(r.kind === "usb_notecard" ? "USB notecard" : "Grid"));
  tr.appendChild(td(r.created_by_legacy_name));
  tr.appendChild(td(fmtDate(r.created_at)));
  const actions = document.createElement("td");
  if (r.status === "done") {
    const dl = document.createElement("a");
    dl.href = `/api/renders/${r.render_id}/download`;
    dl.textContent = "Download";
    dl.className = "row-action";
    actions.appendChild(dl);
    if (r.has_without_route) {
      const dl2 = document.createElement("a");
      dl2.href = `/api/renders/${r.render_id}/image-without-route`;
      dl2.textContent = "No-route image";
      dl2.className = "row-action";
      actions.appendChild(dl2);
    }
    const meta = document.createElement("a");
    meta.href = `/api/renders/${r.render_id}/metadata`;
    meta.textContent = "Metadata";
    meta.className = "row-action";
    actions.appendChild(meta);
    const regen = document.createElement("a");
    regen.href = `/?regenerate=${r.render_id}`;
    regen.textContent = "Regenerate";
    regen.className = "row-action";
    actions.appendChild(regen);
  }
  const del = document.createElement("button");
  del.type = "button";
  del.textContent = "Delete";
  del.className = "row-action danger";
  del.addEventListener("click", async () => {
    if (!confirm("Delete this render?")) return;
    const resp = await fetch(`/api/renders/${r.render_id}`, {
      method: "DELETE",
    });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    refresh();
  });
  actions.appendChild(del);
  tr.appendChild(actions);
  return tr;
}

function td(text) {
  const el = document.createElement("td");
  el.textContent = text;
  return el;
}

let pollingTimer = null;

async function refresh() {
  const scope = $("scope-select").value;
  try {
    const ncs = await fetchJSON(
      `/api/notecards?scope=${encodeURIComponent(scope)}`,
    );
    const ncsBody = $("notecards-tbody");
    ncsBody.replaceChildren();
    if ((ncs.notecards || []).length === 0) {
      $("notecards-status").textContent = "No notecards in this scope.";
    } else {
      $("notecards-status").textContent = "";
      for (const n of ncs.notecards) ncsBody.appendChild(notecardRow(n));
    }
  } catch (err) {
    $("notecards-status").textContent =
      `Failed to load notecards: ${err.message}`;
  }

  try {
    const rrs = await fetchJSON(
      `/api/renders?scope=${encodeURIComponent(scope)}`,
    );
    const rrsBody = $("renders-tbody");
    rrsBody.replaceChildren();
    if ((rrs.renders || []).length === 0) {
      $("renders-status").textContent = "No renders in this scope.";
    } else {
      $("renders-status").textContent = "";
      for (const r of rrs.renders) rrsBody.appendChild(renderRow(r));
    }
    // If anything is still in progress, poll again in 5s.
    if (pollingTimer) clearTimeout(pollingTimer);
    if ((rrs.renders || []).some((r) => r.status === "in_progress")) {
      pollingTimer = setTimeout(refresh, 5000);
    }
  } catch (err) {
    $("renders-status").textContent = `Failed to load renders: ${err.message}`;
  }
}

document.addEventListener("DOMContentLoaded", async () => {
  await populateScopes();
  refresh();
});
