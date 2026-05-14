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

function fmtCoord(x, y) {
  if (x === null || x === undefined || y === null || y === undefined) return "";
  return `${x}, ${y}`;
}

// Build a `<td>` containing a link to a user's profile, or a `(deleted user)`
// placeholder when the underlying account has been removed (uploader /
// creator FKs are `ON DELETE SET NULL`, so both the id and the display
// name come back as null in that case).
function profileLinkCell(userId, legacyName) {
  const cell = document.createElement("td");
  if (!userId) {
    cell.textContent = "(deleted user)";
    cell.className = "muted";
    return cell;
  }
  const a = document.createElement("a");
  a.href = `/profile/${encodeURIComponent(userId)}`;
  a.textContent = legacyName || "(unknown)";
  cell.appendChild(a);
  return cell;
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
  tr.appendChild(
    td(
      n.waypoint_count === null || n.waypoint_count === undefined
        ? ""
        : String(n.waypoint_count),
    ),
  );
  tr.appendChild(td(n.start_region || ""));
  tr.appendChild(td(n.end_region || ""));
  tr.appendChild(td(fmtCoord(n.lower_left_x, n.lower_left_y)));
  tr.appendChild(td(fmtCoord(n.upper_right_x, n.upper_right_y)));
  tr.appendChild(profileLinkCell(n.uploaded_by, n.uploaded_by_legacy_name));
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
    const ok = await confirmModal({
      title: "Delete notecard",
      message: `Delete notecard "${n.name}"?`,
      danger: true,
      okText: "Delete",
    });
    if (!ok) return;
    const resp = await fetch(`/api/notecards/${n.notecard_id}`, {
      method: "DELETE",
    });
    if (!resp.ok) {
      await showError(resp);
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
  tr.appendChild(td(r.notecard_name || ""));
  tr.appendChild(td(fmtCoord(r.lower_left_x, r.lower_left_y)));
  tr.appendChild(td(fmtCoord(r.upper_right_x, r.upper_right_y)));
  tr.appendChild(profileLinkCell(r.created_by, r.created_by_legacy_name));
  tr.appendChild(td(fmtDate(r.created_at)));
  const actions = document.createElement("td");
  if (r.status === "done") {
    const view = document.createElement("button");
    view.type = "button";
    view.textContent = "View";
    view.className = "row-action";
    view.addEventListener("click", () =>
      showImageModal(r.render_id, r.has_without_route),
    );
    actions.appendChild(view);
    const dl = document.createElement("a");
    dl.href = `/api/renders/${r.render_id}/download`;
    dl.textContent = "Download";
    dl.className = "row-action";
    actions.appendChild(dl);
    if (r.has_without_route) {
      const dl2 = document.createElement("a");
      dl2.href = `/api/renders/${r.render_id}/download-without-route`;
      dl2.textContent = "Download (no route)";
      dl2.className = "row-action";
      actions.appendChild(dl2);
    }
    const meta = document.createElement("button");
    meta.type = "button";
    meta.textContent = "Metadata";
    meta.className = "row-action";
    meta.addEventListener("click", () => showMetadataModal(r.render_id));
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
    const ok = await confirmModal({
      title: "Delete render",
      message: "Delete this render?",
      danger: true,
      okText: "Delete",
    });
    if (!ok) return;
    const resp = await fetch(`/api/renders/${r.render_id}`, {
      method: "DELETE",
    });
    if (!resp.ok) {
      await showError(resp);
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

// Open the image-viewer modal for a saved render. When `hasWithoutRoute`
// is true, both variants are loaded and a small tab strip switches
// between them. The two `<img>` elements are stacked in the same
// position so the inactive one toggles `display:none` and the visible
// one stays put — handy for eyeball comparison of with-route vs
// without-route. The Download button in the modal footer follows the
// active tab.
async function showImageModal(renderId, hasWithoutRoute) {
  const variants = [
    {
      key: "with",
      label: "With route",
      imageUrl: `/api/renders/${renderId}/image`,
      downloadUrl: `/api/renders/${renderId}/download`,
    },
  ];
  if (hasWithoutRoute) {
    variants.push({
      key: "without",
      label: "Without route",
      imageUrl: `/api/renders/${renderId}/image-without-route`,
      downloadUrl: `/api/renders/${renderId}/download-without-route`,
    });
  }
  const downloadLink = document.createElement("a");
  downloadLink.className = "modal-btn";
  downloadLink.textContent = "Download";
  downloadLink.setAttribute("download", "");
  downloadLink.href = variants[0].downloadUrl;
  await infoModal({
    title: "Render",
    footerExtras: [downloadLink],
    build: (dialog) => {
      const imgWrap = document.createElement("div");
      imgWrap.className = "image-modal-wrap";
      const imgs = variants.map((v, i) => {
        const img = document.createElement("img");
        img.className = "image-modal-img";
        img.src = v.imageUrl;
        img.alt = `Render (${v.label})`;
        if (i !== 0) img.classList.add("hidden");
        return img;
      });
      for (const img of imgs) imgWrap.appendChild(img);

      if (variants.length > 1) {
        const tabs = document.createElement("nav");
        tabs.className = "image-modal-tabs";
        const buttons = variants.map((v, i) => {
          const btn = document.createElement("button");
          btn.type = "button";
          btn.className = i === 0 ? "subtab active" : "subtab";
          btn.textContent = v.label;
          btn.addEventListener("click", () => {
            for (let j = 0; j < variants.length; j++) {
              imgs[j].classList.toggle("hidden", j !== i);
              buttons[j].classList.toggle("active", j === i);
            }
            downloadLink.href = variants[i].downloadUrl;
          });
          return btn;
        });
        for (const btn of buttons) tabs.appendChild(btn);
        dialog.appendChild(tabs);
      }
      dialog.appendChild(imgWrap);
    },
  });
}

async function showMetadataModal(renderId) {
  let meta;
  try {
    meta = await fetchJSON(`/api/renders/${renderId}/metadata`);
  } catch (err) {
    await alertModal({
      title: "Metadata",
      message: `Failed to load metadata: ${err.message}`,
    });
    return;
  }
  await infoModal({
    title: "Render metadata",
    build: (dialog) => {
      const aspect = document.createElement("dl");
      aspect.className = "metadata-list";
      const rows = [
        ["Width (regions)", meta.aspect_x],
        ["Height (regions)", meta.aspect_y],
        [
          "Aspect ratio",
          typeof meta.aspect_ratio === "number"
            ? meta.aspect_ratio.toFixed(4)
            : meta.aspect_ratio,
        ],
      ];
      for (const [label, value] of rows) {
        const dt = document.createElement("dt");
        dt.textContent = label;
        const dd = document.createElement("dd");
        dd.textContent = String(value);
        aspect.appendChild(dt);
        aspect.appendChild(dd);
      }
      dialog.appendChild(aspect);

      const ppsHeading = document.createElement("h3");
      ppsHeading.className = "metadata-subheading";
      ppsHeading.textContent = "PPS HUD config";
      dialog.appendChild(ppsHeading);

      const ppsBox = document.createElement("textarea");
      ppsBox.className = "metadata-pps";
      ppsBox.readOnly = true;
      ppsBox.value = meta.pps_hud_config || "";
      ppsBox.rows = 3;
      dialog.appendChild(ppsBox);

      const copyRow = document.createElement("div");
      copyRow.className = "metadata-copy-row";
      const copyBtn = document.createElement("button");
      copyBtn.type = "button";
      copyBtn.className = "modal-btn";
      copyBtn.textContent = "Copy config";
      const copyStatus = document.createElement("span");
      copyStatus.className = "metadata-copy-status";
      copyBtn.addEventListener("click", async () => {
        try {
          await navigator.clipboard.writeText(ppsBox.value);
          copyStatus.textContent = "Copied.";
        } catch (_err) {
          ppsBox.select();
          copyStatus.textContent = "Copy failed — selected for manual copy.";
        }
      });
      copyRow.appendChild(copyBtn);
      copyRow.appendChild(copyStatus);
      dialog.appendChild(copyRow);

      const howHeading = document.createElement("h3");
      howHeading.className = "metadata-subheading";
      howHeading.textContent = "How to apply this to your PPS HUD";
      dialog.appendChild(howHeading);

      const steps = document.createElement("ol");
      steps.className = "metadata-steps";
      const items = [
        "Upload the rendered map image to Second Life and apply it to the map face of the PPS.",
        "Resize the PPS so its display matches the aspect ratio shown above.",
        'Edit your PPS and enable "Edit Linked Parts".',
        "Select the dot prim and paste the config above into its description.",
        "Long-click the dot prim and choose Reset in the menu that appears.",
      ];
      for (const text of items) {
        const li = document.createElement("li");
        li.textContent = text;
        steps.appendChild(li);
      }
      dialog.appendChild(steps);
    },
  });
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
