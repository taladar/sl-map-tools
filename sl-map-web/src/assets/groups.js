// /groups and /groups/{id} page driver.

function $(id) {
  return document.getElementById(id);
}

function fmtDate(iso) {
  if (!iso) return "";
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? iso : d.toLocaleString();
}

function td(text) {
  const el = document.createElement("td");
  el.textContent = text;
  return el;
}

async function fetchJSON(url, init) {
  const resp = await fetch(url, init);
  if (!resp.ok) throw new Error(await resp.text());
  return resp.json();
}

function parseGroupIdFromPath() {
  const m = window.location.pathname.match(/^\/groups\/([0-9a-fA-F-]{36})/);
  return m ? m[1] : null;
}

async function showList() {
  $("groups-list-section").classList.remove("hidden");
  $("group-detail-section").classList.add("hidden");
  try {
    const data = await fetchJSON("/api/groups");
    const tbody = $("groups-tbody");
    tbody.replaceChildren();
    if (!data.groups || data.groups.length === 0) {
      $("groups-status").textContent = "You are not in any groups yet.";
      return;
    }
    $("groups-status").textContent = "";
    for (const g of data.groups) {
      const tr = document.createElement("tr");
      const nameTd = document.createElement("td");
      const a = document.createElement("a");
      a.href = `/groups/${g.group_id}`;
      a.textContent = g.name;
      nameTd.appendChild(a);
      tr.appendChild(nameTd);
      tr.appendChild(td(g.my_role));
      tr.appendChild(td(fmtDate(g.created_at)));
      const actions = document.createElement("td");
      const open = document.createElement("a");
      open.href = `/groups/${g.group_id}`;
      open.textContent = "Open";
      open.className = "row-action";
      actions.appendChild(open);
      tr.appendChild(actions);
      tbody.appendChild(tr);
    }
  } catch (err) {
    $("groups-status").textContent = `Failed to load groups: ${err.message}`;
  }
}

$("create-group-form").addEventListener("submit", async (e) => {
  e.preventDefault();
  const name = $("new-group-name").value.trim();
  if (!name) return;
  try {
    const resp = await fetch("/api/groups", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name }),
    });
    if (!resp.ok) throw new Error(await resp.text());
    const { group } = await resp.json();
    window.location.assign(`/groups/${group.group_id}`);
  } catch (err) {
    $("groups-status").textContent = `Failed: ${err.message}`;
  }
});

async function showDetail(groupId) {
  $("groups-list-section").classList.add("hidden");
  $("group-detail-section").classList.remove("hidden");
  $("library-link").href = `/library`;
  sessionStorage.setItem("sl-map-web.library.scope", `group:${groupId}`);

  let group;
  try {
    const g = await fetchJSON(`/api/groups/${groupId}`);
    group = g.group;
    $("group-detail-name").textContent = group.name;
    $("group-detail-status").textContent =
      `Role: ${group.my_role} · created ${fmtDate(group.created_at)}`;
  } catch (err) {
    $("group-detail-status").textContent = `Failed: ${err.message}`;
    return;
  }

  const isOwner = group.my_role === "owner";
  $("owner-actions").classList.toggle("hidden", !isOwner);

  try {
    const ms = await fetchJSON(`/api/groups/${groupId}/members`);
    const tbody = $("members-tbody");
    tbody.replaceChildren();
    for (const m of ms.members || []) {
      const tr = document.createElement("tr");
      tr.appendChild(td(m.username));
      tr.appendChild(td(m.legacy_name));
      tr.appendChild(td(m.role));
      const actions = document.createElement("td");
      if (isOwner) {
        if (m.role === "member") {
          const promote = document.createElement("button");
          promote.type = "button";
          promote.className = "row-action";
          promote.textContent = "Promote";
          promote.addEventListener("click", () =>
            patchRole(groupId, m.user_id, "owner"),
          );
          actions.appendChild(promote);
        } else {
          const demote = document.createElement("button");
          demote.type = "button";
          demote.className = "row-action";
          demote.textContent = "Demote";
          demote.addEventListener("click", () =>
            patchRole(groupId, m.user_id, "member"),
          );
          actions.appendChild(demote);
        }
        const kick = document.createElement("button");
        kick.type = "button";
        kick.className = "row-action danger";
        kick.textContent = "Remove";
        kick.addEventListener("click", async () => {
          if (!confirm(`Remove ${m.legacy_name} from this group?`)) return;
          const resp = await fetch(
            `/api/groups/${groupId}/members/${m.user_id}`,
            { method: "DELETE" },
          );
          if (!resp.ok) {
            alert(await resp.text());
            return;
          }
          showDetail(groupId);
        });
        actions.appendChild(kick);
      }
      tr.appendChild(actions);
      tbody.appendChild(tr);
    }
  } catch (err) {
    console.error(err);
  }

  if (isOwner) {
    try {
      const invs = await fetchJSON(`/api/groups/${groupId}/invitations`);
      const tbody = $("invitations-tbody");
      tbody.replaceChildren();
      for (const i of invs.invitations || []) {
        const tr = document.createElement("tr");
        tr.appendChild(td(i.invitee_legacy_name));
        tr.appendChild(td(i.target_role));
        tr.appendChild(td(fmtDate(i.created_at)));
        tr.appendChild(td(i.status));
        tbody.appendChild(tr);
      }
    } catch (err) {
      console.error(err);
    }
  }

  $("invite-form").onsubmit = async (e) => {
    e.preventDefault();
    const identifier = $("invite-identifier").value.trim();
    if (!identifier) return;
    const role = $("invite-role").value;
    const resp = await fetch(`/api/groups/${groupId}/invitations`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ identifier, target_role: role }),
    });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    $("invite-identifier").value = "";
    showDetail(groupId);
  };

  $("rename-group").onclick = async () => {
    const name = prompt("New group name?", group.name);
    if (!name) return;
    const resp = await fetch(`/api/groups/${groupId}`, {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ name }),
    });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    showDetail(groupId);
  };

  $("delete-group").onclick = async () => {
    if (
      !confirm(
        `Delete group "${group.name}"? This cannot be undone and will remove all group-owned notecards and renders.`,
      )
    )
      return;
    const resp = await fetch(`/api/groups/${groupId}`, { method: "DELETE" });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    window.location.assign("/groups");
  };

  $("leave-group").onclick = async () => {
    if (!confirm(`Leave "${group.name}"?`)) return;
    const resp = await fetch(`/api/groups/${groupId}/leave`, {
      method: "POST",
    });
    if (!resp.ok) {
      alert(await resp.text());
      return;
    }
    window.location.assign("/groups");
  };
}

async function patchRole(groupId, userId, role) {
  const resp = await fetch(`/api/groups/${groupId}/members/${userId}`, {
    method: "PATCH",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ role }),
  });
  if (!resp.ok) {
    alert(await resp.text());
    return;
  }
  showDetail(groupId);
}

document.addEventListener("DOMContentLoaded", () => {
  const id = parseGroupIdFromPath();
  if (id) showDetail(id);
  else showList();
});
