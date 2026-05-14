// sl-map-web — vanilla JS frontend.
//
// Composition strategy for the preview: we know the SL map CDN URL pattern
// (https://secondlife-maps-cdn.akamaized.net/map-{z}-{x}-{y}-objects.jpg)
// and the zoom-level → regions-per-tile / pixels-per-region mapping that
// `sl-types::map::ZoomLevel` defines. We pick the highest-detail zoom that
// keeps the preview under ~1024×1024 and drop `<img>` tags positioned in
// region space. No tiles flow through our server.

// Strict 8-4-4-4-12 hex form, matching the canonical UUID layout emitted
// by the server. Used to validate UUID-shaped query params before they
// are interpolated into fetch URLs or assigned to form fields.
const UUID_RE =
  /^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$/;
function isUuid(s) {
  return typeof s === "string" && UUID_RE.test(s);
}

// --- auth: redirect to /login on 401 and populate the header bar ---

function redirectToLogin() {
  const next = encodeURIComponent(
    window.location.pathname + window.location.search,
  );
  window.location.assign(`/login?next=${next}`);
}

const _originalFetch = window.fetch.bind(window);
window.fetch = async (...args) => {
  const resp = await _originalFetch(...args);
  if (resp.status === 401) {
    redirectToLogin();
  }
  return resp;
};

async function loadCurrentUser() {
  try {
    const resp = await _originalFetch("/api/auth/me");
    if (resp.status === 401) {
      redirectToLogin();
      return;
    }
    if (!resp.ok) return;
    const me = await resp.json();
    const label = document.getElementById("logged-in-as");
    if (label) label.textContent = `Logged in as ${me.legacy_name}`;
    const logout = document.getElementById("logout-button");
    if (logout) logout.classList.remove("hidden");
  } catch (_err) {
    // network failures during the optional "who am I" call shouldn't block
    // the rest of the page from initialising
  }
}

document.addEventListener("DOMContentLoaded", () => {
  loadCurrentUser();
  const logout = document.getElementById("logout-button");
  if (logout) {
    logout.addEventListener("click", async () => {
      try {
        await _originalFetch("/api/auth/logout", { method: "POST" });
      } catch (_err) {
        // ignore network errors; the redirect below either way clears UI
      }
      window.location.assign("/login");
    });
  }
  decorateInvitationsLink();
});

// Populate the invitations nav link with a count of pending invites. Called
// on every page that uses app.js.
async function decorateInvitationsLink() {
  const link = document.getElementById("invitations-link");
  if (!link) return;
  try {
    const resp = await _originalFetch("/api/invitations");
    if (!resp.ok) return;
    const data = await resp.json();
    const n = (data.invitations || []).length;
    if (n > 0) {
      const badge = document.createElement("span");
      badge.className = "badge";
      badge.textContent = String(n);
      link.appendChild(document.createTextNode(" "));
      link.appendChild(badge);
    }
  } catch (_err) {
    // ignore
  }
}

const TILE_URL = (z, x, y) =>
  `https://secondlife-maps-cdn.akamaized.net/map-${z}-${x}-${y}-objects.jpg`;

// tile_size(z) = 2^(z-1) regions per tile (matches sl-types ZoomLevel::tile_size)
const tileSize = (z) => 1 << (z - 1);
// pixels_per_region(z) = 2^(9-z)
const pixelsPerRegion = (z) => 1 << (9 - z);
// every SL map tile is 256×256 px
const TILE_PX = 256;

const PREVIEW_BUDGET_PX = 1024;

function pickPreviewZoom(sizeX, sizeY) {
  for (let z = 1; z <= 8; z++) {
    if (
      sizeX * pixelsPerRegion(z) <= PREVIEW_BUDGET_PX &&
      sizeY * pixelsPerRegion(z) <= PREVIEW_BUDGET_PX
    ) {
      return z;
    }
  }
  return 8;
}

function $(id) {
  return document.getElementById(id);
}

function show(el) {
  el.classList.remove("hidden");
}

function hide(el) {
  el.classList.add("hidden");
}

// --- tab switching ---

document.querySelectorAll(".tab").forEach((tab) => {
  tab.addEventListener("click", () => {
    document.querySelectorAll(".tab").forEach((t) => {
      t.classList.remove("active");
    });
    document.querySelectorAll(".tab-panel").forEach((p) => {
      p.classList.remove("active");
    });
    tab.classList.add("active");
    const panel = document.getElementById(`tab-${tab.dataset.tab}`);
    if (panel) panel.classList.add("active");
  });
});

// --- shared param helpers ---

$("missing_map_tile_enabled").addEventListener("change", (e) => {
  $("missing_map_tile_color").disabled = !e.target.checked;
});
$("missing_region_enabled").addEventListener("change", (e) => {
  $("missing_region_color").disabled = !e.target.checked;
});

// --- destination + saved-notecard pickers ---

async function loadGroupsAndNotecards() {
  let groups = { groups: [] };
  try {
    const r = await fetch("/api/groups");
    if (r.ok) groups = await r.json();
  } catch (_err) {
    // leave empty
  }
  const saveTo = $("save_to");
  if (saveTo) {
    for (const g of groups.groups || []) {
      if (g.my_role !== "owner") continue;
      const o = document.createElement("option");
      o.value = `group:${g.group_id}`;
      o.textContent = `Group: ${g.name}`;
      saveTo.appendChild(o);
    }
  }
  const ncPicker = $("notecard_id");
  if (ncPicker) {
    // List notecards across personal + member groups (any scope the user can
    // view).
    const seen = new Set();
    const scopes = ["personal"];
    for (const g of groups.groups || []) scopes.push(`group:${g.group_id}`);
    for (const scope of scopes) {
      try {
        const r = await fetch(
          `/api/notecards?scope=${encodeURIComponent(scope)}`,
        );
        if (!r.ok) continue;
        const data = await r.json();
        for (const n of data.notecards || []) {
          if (seen.has(n.notecard_id)) continue;
          seen.add(n.notecard_id);
          const o = document.createElement("option");
          o.value = n.notecard_id;
          o.textContent = `${n.name} (${scope === "personal" ? "personal" : "group"})`;
          ncPicker.appendChild(o);
        }
      } catch (_err) {
        // ignore
      }
    }
  }
}

document.addEventListener("DOMContentLoaded", loadGroupsAndNotecards);

function readSharedParams() {
  return {
    max_width: parseInt($("max_width").value, 10),
    max_height: parseInt($("max_height").value, 10),
    missing_map_tile_color: $("missing_map_tile_enabled").checked
      ? $("missing_map_tile_color").value
      : null,
    missing_region_color: $("missing_region_enabled").checked
      ? $("missing_region_color").value
      : null,
    format: $("format").value,
  };
}

function readBorders() {
  const get = (id) => {
    const v = $(id).value.trim();
    return v === "" ? null : parseInt(v, 10);
  };
  return {
    border_regions: get("border_regions"),
    border_north: get("border_north"),
    border_south: get("border_south"),
    border_east: get("border_east"),
    border_west: get("border_west"),
  };
}

function appendBordersToForm(fd) {
  const b = readBorders();
  Object.entries(b).forEach(([k, v]) => {
    if (v !== null) fd.append(k, String(v));
  });
}

// --- preview composition ---

function renderPreview(rect, waypoints) {
  const container = $("preview-container");
  container.replaceChildren();
  const sizeX = rect.upper_right_x - rect.lower_left_x + 1;
  const sizeY = rect.upper_right_y - rect.lower_left_y + 1;
  if (sizeX <= 0 || sizeY <= 0) {
    $("preview-status").textContent =
      "Invalid rectangle: corners must be ordered.";
    return;
  }
  const z = pickPreviewZoom(sizeX, sizeY);
  const ts = tileSize(z);
  // align to tile boundaries
  const firstX = rect.lower_left_x - (rect.lower_left_x % ts);
  const firstY = rect.lower_left_y - (rect.lower_left_y % ts);
  const lastX = rect.upper_right_x - (rect.upper_right_x % ts);
  const lastY = rect.upper_right_y - (rect.upper_right_y % ts);
  const tilesX = (lastX - firstX) / ts + 1;
  const tilesY = (lastY - firstY) / ts + 1;
  const widthPx = tilesX * TILE_PX;
  const heightPx = tilesY * TILE_PX;

  // Build everything inside a viewport so we can scale the whole thing
  // (tiles + route overlay together) with a single CSS transform if the
  // native tile-grid dimensions exceed the available container width.
  const viewport = document.createElement("div");
  viewport.className = "viewport";
  viewport.style.width = `${widthPx}px`;
  viewport.style.height = `${heightPx}px`;
  // store intrinsic dimensions on the DOM node so the resize listener can
  // re-fit without closing over the current call's locals
  viewport.dataset.intrinsicWidth = String(widthPx);
  viewport.dataset.intrinsicHeight = String(heightPx);

  const tiles = document.createElement("div");
  tiles.className = "tiles";
  tiles.style.width = `${widthPx}px`;
  tiles.style.height = `${heightPx}px`;
  for (let tx = 0; tx < tilesX; tx++) {
    for (let ty = 0; ty < tilesY; ty++) {
      const cornerX = firstX + tx * ts;
      const cornerY = firstY + ty * ts;
      const img = document.createElement("img");
      img.className = "tile";
      img.loading = "lazy";
      img.alt = `tile ${z}-${cornerX}-${cornerY}`;
      img.src = TILE_URL(z, cornerX, cornerY);
      img.style.left = `${tx * TILE_PX}px`;
      // SL y increases upward but DOM y increases downward
      img.style.top = `${(tilesY - 1 - ty) * TILE_PX}px`;
      tiles.appendChild(img);
    }
  }
  viewport.appendChild(tiles);

  if (waypoints && waypoints.length > 1) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.classList.add("route-overlay");
    svg.setAttribute("viewBox", `0 0 ${widthPx} ${heightPx}`);
    svg.setAttribute("width", widthPx);
    svg.setAttribute("height", heightPx);
    const ppRegion = pixelsPerRegion(z);
    const ppMeter = ppRegion / 256;
    const points = waypoints
      .map((w) => {
        const px = (w.region_x - firstX) * ppRegion + w.x * ppMeter;
        const py =
          heightPx - ((w.region_y - firstY) * ppRegion + w.y * ppMeter);
        return `${px.toFixed(1)},${py.toFixed(1)}`;
      })
      .join(" ");
    const polyline = document.createElementNS(
      "http://www.w3.org/2000/svg",
      "polyline",
    );
    polyline.setAttribute("points", points);
    polyline.setAttribute("fill", "none");
    polyline.setAttribute("stroke", $("route_color").value);
    polyline.setAttribute("stroke-width", "3");
    svg.appendChild(polyline);
    viewport.appendChild(svg);
  }

  container.appendChild(viewport);
  fitViewport(container, viewport, widthPx, heightPx);

  $("preview-status").textContent =
    `Preview at zoom ${z} (${pixelsPerRegion(z)} px/region) — ` +
    `${tilesX * tilesY} tile${tilesX * tilesY === 1 ? "" : "s"}, ` +
    `${widthPx}×${heightPx} px.`;
}

// Scale the viewport so its native pixel size fits within the container's
// available width and a sensible max height (70% of viewport height). Only
// scales down — when the tile grid is already small enough it is shown at
// 1:1.
function fitViewport(container, viewport, widthPx, heightPx) {
  const availWidth = container.clientWidth || widthPx;
  const maxHeight = Math.max(window.innerHeight * 0.7, 400);
  const scale = Math.min(availWidth / widthPx, maxHeight / heightPx, 1);
  viewport.style.transformOrigin = "0 0";
  viewport.style.transform = `scale(${scale})`;
  // ensure the parent reserves the right amount of space so the page
  // doesn't overflow and the layout below the preview stays in flow
  container.style.height = `${heightPx * scale}px`;
}

// re-fit any visible map containers on window resize. We re-read the
// intrinsic dimensions from the viewport's dataset so this works even
// after the preview has been regenerated with a different rectangle.
window.addEventListener("resize", () => {
  document.querySelectorAll(".map-container").forEach((container) => {
    const vp = container.querySelector(".viewport");
    if (!vp) return;
    const w = parseFloat(vp.dataset.intrinsicWidth);
    const h = parseFloat(vp.dataset.intrinsicHeight);
    if (Number.isFinite(w) && Number.isFinite(h) && w > 0 && h > 0) {
      fitViewport(container, vp, w, h);
    }
  });
});

// --- preview handlers ---

$("grid_preview").addEventListener("click", () => {
  const rect = {
    lower_left_x: parseInt($("ll_x").value, 10),
    lower_left_y: parseInt($("ll_y").value, 10),
    upper_right_x: parseInt($("ur_x").value, 10),
    upper_right_y: parseInt($("ur_y").value, 10),
  };
  renderPreview(rect, null);
});

async function buildNotecardForm() {
  const fd = new FormData();
  const file = $("notecard_file").files[0];
  const text = $("notecard_text").value;
  if (file) {
    fd.append("notecard", file);
  } else if (text.trim() !== "") {
    fd.append("notecard_text", text);
  } else {
    throw new Error("supply either a notecard file or pasted text");
  }
  appendBordersToForm(fd);
  return fd;
}

$("notecard_preview").addEventListener("click", async () => {
  $("preview-status").textContent = "Resolving notecard…";
  try {
    const fd = await buildNotecardForm();
    const resp = await fetch("/api/notecard/derive-rectangle", {
      method: "POST",
      body: fd,
    });
    if (!resp.ok) throw new Error(await resp.text());
    const data = await resp.json();
    renderPreview(data, data.waypoints);
  } catch (err) {
    $("preview-status").textContent = `Preview failed: ${err.message}`;
  }
});

// --- render handlers ---

const tileGridEl = $("tile-grid");
const renderProgressEl = $("render-progress");
const renderResultEl = $("render-result");
const renderStatusEl = $("render-status");

function startRenderUI() {
  hide(renderResultEl);
  show(renderProgressEl);
  tileGridEl.replaceChildren();
  totalTiles = 0;
  finishedTiles = 0;
  totalRegions = 0;
  checkedRegions = 0;
  totalWaypoints = 0;
  resolvedWaypoints = 0;
  renderStatusEl.textContent = "Starting render…";
}

const tileCells = new Map();

function tileKey(z, x, y) {
  return `${z}-${x}-${y}`;
}

function ensureTileCell(z, x, y) {
  const key = tileKey(z, x, y);
  let cell = tileCells.get(key);
  if (!cell) {
    cell = document.createElement("span");
    cell.className = "tile-cell";
    cell.title = key;
    tileGridEl.appendChild(cell);
    tileCells.set(key, cell);
  }
  return cell;
}

let totalTiles = 0;
let finishedTiles = 0;
let totalRegions = 0;
let checkedRegions = 0;
let totalWaypoints = 0;
let resolvedWaypoints = 0;

function updateStatus() {
  const parts = [];
  if (totalTiles > 0) {
    parts.push(`tiles: ${finishedTiles} / ${totalTiles}`);
  }
  if (totalRegions > 0) {
    parts.push(`region checks: ${checkedRegions} / ${totalRegions}`);
  }
  if (totalWaypoints > 0) {
    parts.push(`waypoints: ${resolvedWaypoints} / ${totalWaypoints}`);
  }
  renderStatusEl.textContent = parts.join("  ·  ");
}

function handleProgress(evt) {
  switch (evt.type) {
    case "plan_computed":
      totalTiles = evt.total_tiles;
      finishedTiles = 0;
      tileCells.clear();
      tileGridEl.replaceChildren();
      updateStatus();
      break;
    case "tile_started": {
      const cell = ensureTileCell(evt.zoom, evt.x, evt.y);
      cell.classList.add("active");
      break;
    }
    case "tile_finished": {
      const cell = ensureTileCell(evt.zoom, evt.x, evt.y);
      cell.classList.remove("active");
      cell.classList.add(evt.outcome);
      finishedTiles += 1;
      updateStatus();
      break;
    }
    case "region_check_planned":
      totalRegions = evt.total_regions;
      checkedRegions = 0;
      updateStatus();
      break;
    case "region_checked":
      checkedRegions += 1;
      // updating the status text on every region check would cause a lot
      // of DOM churn for large rectangles; throttle to one refresh per
      // ~32 checks plus the final one (handled by `done`)
      if (checkedRegions === totalRegions || (checkedRegions & 0x1f) === 0) {
        updateStatus();
      }
      break;
    case "route_planned":
      totalWaypoints = evt.total_waypoints;
      resolvedWaypoints = 0;
      updateStatus();
      break;
    case "route_waypoint_resolved":
      resolvedWaypoints = evt.index + 1;
      updateStatus();
      break;
    case "done":
      renderStatusEl.textContent = "Render complete.";
      break;
    case "error":
      renderStatusEl.textContent = `Render failed: ${evt.message}`;
      break;
    default:
      break;
  }
}

async function followJob(jobId, withWithoutRoute) {
  return new Promise((resolve, reject) => {
    const source = new EventSource(`/api/render/${jobId}/events`);
    let failedMessage = null;
    source.onmessage = (ev) => {
      try {
        const evt = JSON.parse(ev.data);
        handleProgress(evt);
        if (evt.type === "error") {
          failedMessage = evt.message;
        }
        if (evt.type === "done" || evt.type === "error") {
          source.close();
          if (failedMessage) reject(new Error(failedMessage));
          else resolve();
        }
      } catch (_err) {
        // ignore malformed events
      }
    };
    source.onerror = () => {
      // EventSource fires onerror on close too; if we haven't resolved we
      // give the server one more chance via the result endpoint
      source.close();
      resolve();
    };
  }).then(async () => {
    const metaResp = await fetch(`/api/render/${jobId}/metadata`);
    if (!metaResp.ok) throw new Error(await metaResp.text());
    const meta = await metaResp.json();
    showResult(jobId, meta, withWithoutRoute);
  });
}

function showResult(jobId, meta, withWithoutRoute) {
  hide(renderProgressEl);
  show(renderResultEl);
  const ratioStr = (meta.aspect_ratio || 0).toFixed(4);
  $("render-metadata").textContent =
    `Aspect ratio: ${meta.aspect_x}:${meta.aspect_y} (${ratioStr}). ` +
    `PPS HUD config: ${meta.pps_hud_config}`;
  const img = $("result-image");
  img.src = `/api/render/${jobId}/image`;
  const dl = $("download-image");
  dl.href = img.src;
  dl.download = `sl-map-${jobId}.${$("format").value === "jpeg" ? "jpg" : "png"}`;
  const dlNoRoute = $("download-without-route");
  if (withWithoutRoute) {
    dlNoRoute.href = `/api/render/${jobId}/image-without-route`;
    dlNoRoute.download = `sl-map-no-route-${jobId}.${$("format").value === "jpeg" ? "jpg" : "png"}`;
    show(dlNoRoute);
  } else {
    hide(dlNoRoute);
  }
}

$("grid_render").addEventListener("click", async () => {
  startRenderUI();
  try {
    const body = {
      lower_left_x: parseInt($("ll_x").value, 10),
      lower_left_y: parseInt($("ll_y").value, 10),
      upper_right_x: parseInt($("ur_x").value, 10),
      upper_right_y: parseInt($("ur_y").value, 10),
      ...readSharedParams(),
      save_to: $("save_to").value,
    };
    const resp = await fetch("/api/render/grid-rectangle", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
    });
    if (!resp.ok) throw new Error(await resp.text());
    const { job_id } = await resp.json();
    await followJob(job_id, false);
  } catch (err) {
    renderStatusEl.textContent = `Render failed: ${err.message}`;
  }
});

$("notecard_render").addEventListener("click", async () => {
  startRenderUI();
  try {
    const reuseId = $("notecard_id").value.trim();
    const fd = new FormData();
    if (reuseId !== "") {
      fd.append("notecard_id", reuseId);
    } else {
      const file = $("notecard_file").files[0];
      const text = $("notecard_text").value;
      if (file) {
        fd.append("notecard", file);
      } else if (text.trim() !== "") {
        fd.append("notecard_text", text);
      } else {
        throw new Error(
          "supply either a notecard file, paste text, or reuse a saved notecard",
        );
      }
      const ncName = $("notecard_name").value.trim();
      if (ncName !== "") fd.append("notecard_name", ncName);
    }
    appendBordersToForm(fd);
    const shared = readSharedParams();
    fd.append("max_width", String(shared.max_width));
    fd.append("max_height", String(shared.max_height));
    fd.append("format", shared.format);
    if (shared.missing_map_tile_color) {
      fd.append("missing_map_tile_color", shared.missing_map_tile_color);
    }
    if (shared.missing_region_color) {
      fd.append("missing_region_color", shared.missing_region_color);
    }
    fd.append("color", $("route_color").value);
    fd.append("save_to", $("save_to").value);
    const withWithoutRoute = $("save_without_route").checked;
    if (withWithoutRoute) fd.append("save_without_route", "true");
    const resp = await fetch("/api/render/usb-notecard", {
      method: "POST",
      body: fd,
    });
    if (!resp.ok) throw new Error(await resp.text());
    const { job_id, notecard } = await resp.json();
    if (notecard) addNotecardOptionIfNew(notecard);
    await followJob(job_id, withWithoutRoute);
  } catch (err) {
    renderStatusEl.textContent = `Render failed: ${err.message}`;
  }
});

function addNotecardOptionIfNew({ notecard_id, name, scope }) {
  const ncPicker = $("notecard_id");
  if (!ncPicker) return;
  for (const o of ncPicker.options) {
    if (o.value === notecard_id) return;
  }
  const o = document.createElement("option");
  o.value = notecard_id;
  o.textContent = `${name} (${scope === "personal" ? "personal" : "group"})`;
  ncPicker.appendChild(o);
}

// --- prefill from regenerate / reuse query params ---

async function applyPrefillFromQuery() {
  const params = new URLSearchParams(window.location.search);
  const reuse = params.get("reuse_notecard");
  const regen = params.get("regenerate");
  // Both params flow into either a select-element value or a fetch URL,
  // so a non-UUID payload could shape arbitrary same-origin requests
  // via the user's session. The server already rejects with 404, but
  // we silently drop bad values here so the request is never sent.
  if (isUuid(reuse)) {
    // Wait for the notecard picker to populate; then select.
    setTimeout(() => {
      const sel = $("notecard_id");
      if (sel) {
        sel.value = reuse;
        // switch to the notecard tab
        const tab = document.querySelector('.tab[data-tab="notecard"]');
        if (tab) tab.click();
      }
    }, 300);
  }
  if (isUuid(regen)) {
    try {
      const resp = await fetch(`/api/renders/${regen}/settings`);
      if (!resp.ok) throw new Error(await resp.text());
      const settings = await resp.json();
      applySettings(settings);
    } catch (err) {
      console.error("regenerate prefill failed:", err);
    }
  }
}

function applySettings(s) {
  if (s.kind === "grid_rectangle") {
    $("ll_x").value = s.lower_left_x;
    $("ll_y").value = s.lower_left_y;
    $("ur_x").value = s.upper_right_x;
    $("ur_y").value = s.upper_right_y;
    $("max_width").value = s.max_width;
    $("max_height").value = s.max_height;
    $("format").value = s.format;
    if (s.missing_map_tile_color) {
      $("missing_map_tile_enabled").checked = true;
      $("missing_map_tile_color").disabled = false;
      $("missing_map_tile_color").value = s.missing_map_tile_color;
    }
    if (s.missing_region_color) {
      $("missing_region_enabled").checked = true;
      $("missing_region_color").disabled = false;
      $("missing_region_color").value = s.missing_region_color;
    }
    const tab = document.querySelector('.tab[data-tab="grid"]');
    if (tab) tab.click();
  } else if (s.kind === "usb_notecard") {
    $("max_width").value = s.max_width;
    $("max_height").value = s.max_height;
    $("format").value = s.format;
    if (s.missing_map_tile_color) {
      $("missing_map_tile_enabled").checked = true;
      $("missing_map_tile_color").disabled = false;
      $("missing_map_tile_color").value = s.missing_map_tile_color;
    }
    if (s.missing_region_color) {
      $("missing_region_enabled").checked = true;
      $("missing_region_color").disabled = false;
      $("missing_region_color").value = s.missing_region_color;
    }
    $("border_north").value = s.border_north || "";
    $("border_south").value = s.border_south || "";
    $("border_east").value = s.border_east || "";
    $("border_west").value = s.border_west || "";
    if (s.color) $("route_color").value = s.color;
    $("save_without_route").checked = !!s.save_without_route;
    // Pre-select the source notecard once the picker has populated.
    setTimeout(() => {
      const sel = $("notecard_id");
      if (sel && s.notecard_id) sel.value = s.notecard_id;
    }, 300);
    const tab = document.querySelector('.tab[data-tab="notecard"]');
    if (tab) tab.click();
  }
}

document.addEventListener("DOMContentLoaded", applyPrefillFromQuery);
