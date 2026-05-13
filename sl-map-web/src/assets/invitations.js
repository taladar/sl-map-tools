// /invitations page driver.

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

async function refresh() {
  try {
    const resp = await fetch("/api/invitations");
    if (!resp.ok) throw new Error(await resp.text());
    const data = await resp.json();
    const tbody = $("invitations-tbody");
    tbody.replaceChildren();
    if (!data.invitations || data.invitations.length === 0) {
      $("invitations-status").textContent = "No pending invitations.";
      return;
    }
    $("invitations-status").textContent = "";
    for (const inv of data.invitations) {
      const tr = document.createElement("tr");
      tr.appendChild(td(inv.group_name));
      tr.appendChild(td(inv.inviter_legacy_name));
      tr.appendChild(td(inv.target_role));
      tr.appendChild(td(fmtDate(inv.created_at)));
      const actions = document.createElement("td");
      const accept = document.createElement("button");
      accept.type = "button";
      accept.className = "row-action";
      accept.textContent = "Accept";
      accept.addEventListener("click", async () => {
        const r = await fetch(`/api/invitations/${inv.invitation_id}/accept`, {
          method: "POST",
        });
        if (!r.ok) {
          await showError(r);
          return;
        }
        refresh();
      });
      actions.appendChild(accept);
      const reject = document.createElement("button");
      reject.type = "button";
      reject.className = "row-action danger";
      reject.textContent = "Reject";
      reject.addEventListener("click", async () => {
        const r = await fetch(`/api/invitations/${inv.invitation_id}/reject`, {
          method: "POST",
        });
        if (!r.ok) {
          await showError(r);
          return;
        }
        refresh();
      });
      actions.appendChild(reject);
      tr.appendChild(actions);
      tbody.appendChild(tr);
    }
  } catch (err) {
    $("invitations-status").textContent = `Failed: ${err.message}`;
  }
}

document.addEventListener("DOMContentLoaded", refresh);
