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
    // Apply the saved route-colour preference if one was set. The
    // input only exists on the renderer page; other pages just skip.
    // `applyPrefillFromQuery` runs after this in DOM order and will
    // overwrite the value when a `?regenerate=<id>` link carries an
    // explicit `s.color`, which is the right precedence: regenerate
    // is meant to reproduce the original render, not the user's
    // current preference.
    const routeColor = document.getElementById("route_color");
    if (
      routeColor &&
      typeof me.route_color === "string" &&
      /^#[0-9a-fA-F]{6}$/.test(me.route_color)
    ) {
      routeColor.value = me.route_color;
    }
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

function activateSubtab(name) {
  document.querySelectorAll(".subtab").forEach((t) => {
    t.classList.toggle("active", t.dataset.subtab === name);
  });
  document.querySelectorAll(".subtab-panel").forEach((p) => {
    p.classList.toggle("active", p.id === `subtab-${name}`);
  });
}

document.querySelectorAll(".subtab").forEach((tab) => {
  tab.addEventListener("click", () => activateSubtab(tab.dataset.subtab));
});

function activeSubtab() {
  const t = document.querySelector(".subtab.active");
  return t ? t.dataset.subtab : "file";
}

// --- shared param helpers ---

$("missing_map_tile_enabled").addEventListener("change", (e) => {
  $("missing_map_tile_color").disabled = !e.target.checked;
});
$("missing_region_enabled").addEventListener("change", (e) => {
  $("missing_region_color").disabled = !e.target.checked;
});

// Persist the route colour on the user's account so the preferred
// shade follows the user across browsers and devices. The value is
// loaded from `/api/auth/me` (see `loadCurrentUser` above, which is
// the central place that fetches that endpoint) and saved by `PATCH
// /api/users/me/preferences` on every picker change.
const ROUTE_COLOR_RE = /^#[0-9a-fA-F]{6}$/;
$("route_color").addEventListener("change", async (e) => {
  const value = e.target.value;
  if (!ROUTE_COLOR_RE.test(value)) return;
  try {
    await fetch("/api/users/me/preferences", {
      method: "PATCH",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ route_color: value }),
    });
  } catch (_err) {
    // ignore — network failures here are cosmetic; the local picker
    // value still applies to the next render submission.
  }
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
  const scopeSel = $("reuse_scope");
  if (scopeSel) {
    // Reuse-from scope: personal + every group the user can view (member
    // or owner), since members can see saved notecards.
    for (const g of groups.groups || []) {
      const o = document.createElement("option");
      o.value = `group:${g.group_id}`;
      o.textContent = `Group: ${g.name}`;
      scopeSel.appendChild(o);
    }
    scopeSel.addEventListener("change", () => {
      loadNotecardsForScope(scopeSel.value).catch(() => {});
    });
    await loadNotecardsForScope(scopeSel.value);
  }
}

// Populate the `reuse_notecard_id` select with the notecards in `scope`.
// The dropdown is replaced wholesale on every call so it stays in sync
// with the currently chosen scope.
async function loadNotecardsForScope(scope) {
  const sel = $("reuse_notecard_id");
  if (!sel) return;
  const previous = sel.value;
  sel.replaceChildren();
  let notecards = [];
  try {
    const r = await fetch(`/api/notecards?scope=${encodeURIComponent(scope)}`);
    if (r.ok) {
      const data = await r.json();
      notecards = data.notecards || [];
    }
  } catch (_err) {
    // leave empty
  }
  if (notecards.length === 0) {
    const o = document.createElement("option");
    o.value = "";
    o.textContent = "(no saved notecards in this scope)";
    sel.appendChild(o);
    return;
  }
  for (const n of notecards) {
    const o = document.createElement("option");
    o.value = n.notecard_id;
    o.textContent = n.name;
    sel.appendChild(o);
  }
  // Best-effort restore of the previous selection (lets repeated scope
  // switches keep the same notecard if it exists in both scopes).
  if (previous) {
    const match = Array.from(sel.options).find((o) => o.value === previous);
    if (match) sel.value = previous;
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

  // A single overlay carries both the route polyline and the bounds
  // rectangle so they scale together with the tiles. The preview tiles are
  // aligned to tile boundaries (firstX/firstY), so they can show regions
  // outside the requested rectangle; the rectangle marks the area that will
  // actually appear in the final image.
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.classList.add("route-overlay");
  svg.setAttribute("viewBox", `0 0 ${widthPx} ${heightPx}`);
  svg.setAttribute("width", widthPx);
  svg.setAttribute("height", heightPx);
  const ppRegion = pixelsPerRegion(z);
  const ppMeter = ppRegion / 256;

  // Bounds rectangle. The upper-right corner is inclusive, so the rectangle
  // extends one region past upper_right to cover that region in full. SL y
  // increases upward while DOM/SVG y increases downward, so the top edge is
  // derived from the upper-right corner.
  const boundsX = (rect.lower_left_x - firstX) * ppRegion;
  const boundsY = heightPx - (rect.upper_right_y + 1 - firstY) * ppRegion;
  const boundsW = sizeX * ppRegion;
  const boundsH = sizeY * ppRegion;
  // Dim everything outside the bounds: a full-viewport fill with the
  // rectangle punched out via the even-odd fill rule. Drawn before the
  // outline so the dashed border stays on top.
  const dim = document.createElementNS("http://www.w3.org/2000/svg", "path");
  dim.setAttribute(
    "d",
    `M0 0 H${widthPx} V${heightPx} H0 Z ` +
      `M${boundsX.toFixed(1)} ${boundsY.toFixed(1)} ` +
      `h${boundsW.toFixed(1)} v${boundsH.toFixed(1)} ` +
      `h${(-boundsW).toFixed(1)} Z`,
  );
  dim.setAttribute("fill-rule", "evenodd");
  dim.setAttribute("fill", "#000");
  dim.setAttribute("fill-opacity", "0.5");
  svg.appendChild(dim);

  const boundsRect = document.createElementNS(
    "http://www.w3.org/2000/svg",
    "rect",
  );
  boundsRect.setAttribute("x", boundsX.toFixed(1));
  boundsRect.setAttribute("y", boundsY.toFixed(1));
  boundsRect.setAttribute("width", boundsW.toFixed(1));
  boundsRect.setAttribute("height", boundsH.toFixed(1));
  boundsRect.setAttribute("fill", "none");
  boundsRect.setAttribute("stroke", "#ff2d2d");
  boundsRect.setAttribute("stroke-width", "2");
  boundsRect.setAttribute("stroke-dasharray", "6 4");
  // keep the outline crisp regardless of the viewport's fit-to-width scale
  boundsRect.setAttribute("vector-effect", "non-scaling-stroke");
  svg.appendChild(boundsRect);

  if (waypoints && waypoints.length > 1) {
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
  }

  viewport.appendChild(svg);

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

// Populate a FormData with the notecard source fields for whichever
// subtab is active. Throws if the relevant fields are empty.
function appendNotecardSourceToForm(fd) {
  switch (activeSubtab()) {
    case "file": {
      const file = $("notecard_file").files[0];
      if (!file) throw new Error("choose a notecard file");
      fd.append("notecard", file);
      const ncName = $("notecard_name_file").value.trim();
      if (ncName !== "") fd.append("notecard_name", ncName);
      break;
    }
    case "clipboard": {
      const text = $("notecard_text").value;
      if (text.trim() === "") throw new Error("paste a notecard");
      fd.append("notecard_text", text);
      const ncName = $("notecard_name_paste").value.trim();
      if (ncName !== "") fd.append("notecard_name", ncName);
      break;
    }
    case "reuse": {
      const id = $("reuse_notecard_id").value.trim();
      if (!isUuid(id)) throw new Error("choose a saved notecard");
      fd.append("notecard_id", id);
      break;
    }
    default:
      throw new Error("unknown notecard source");
  }
}

async function buildNotecardForm() {
  const fd = new FormData();
  appendNotecardSourceToForm(fd);
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
    const glw = readGlwOptions();
    const body = {
      lower_left_x: parseInt($("ll_x").value, 10),
      lower_left_y: parseInt($("ll_y").value, 10),
      upper_right_x: parseInt($("ur_x").value, 10),
      upper_right_y: parseInt($("ur_y").value, 10),
      ...readSharedParams(),
      save_to: $("save_to").value,
    };
    if (glw) body.glw = glw;
    const labels = readLabels();
    if (labels.length) body.labels = labels;
    const logos = readLogos();
    if (logos.length) body.logos = logos;
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
    const fd = new FormData();
    appendNotecardSourceToForm(fd);
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
    const glw = readGlwOptions();
    if (glw) fd.append("glw_json", JSON.stringify(glw));
    const labels = readLabels();
    if (labels.length) fd.append("labels_json", JSON.stringify(labels));
    const logos = readLogos();
    if (logos.length) fd.append("logos_json", JSON.stringify(logos));
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

// After the server resolves a freshly uploaded (or auto-copied) notecard,
// surface it in the reuse-from picker so subsequent renders can pick it
// without re-uploading. Only inserts if the active scope matches.
function addNotecardOptionIfNew({ notecard_id, name, scope }) {
  const scopeSel = $("reuse_scope");
  const ncSel = $("reuse_notecard_id");
  if (!scopeSel || !ncSel) return;
  if (scopeSel.value !== scope) return;
  for (const o of ncSel.options) {
    if (o.value === notecard_id) return;
  }
  // Drop the "(no saved notecards...)" placeholder if it is still there.
  if (ncSel.options.length === 1 && ncSel.options[0].value === "") {
    ncSel.replaceChildren();
  }
  const o = document.createElement("option");
  o.value = notecard_id;
  o.textContent = name;
  ncSel.appendChild(o);
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
    selectReuseNotecard(reuse).catch(() => {});
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
    applyGlwSettings(s.glw);
    applyLabels(s.labels);
    applyLogos(s.logos);
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
    if (s.notecard_id) {
      selectReuseNotecard(s.notecard_id).catch(() => {});
    }
    applyGlwSettings(s.glw);
    applyLabels(s.labels);
    applyLogos(s.logos);
    const tab = document.querySelector('.tab[data-tab="notecard"]');
    if (tab) tab.click();
  }
}

// Switch to the reuse subtab and select the given notecard, looking up
// its scope so the scope dropdown can be set first. Looked up via the
// `/api/notecards/{id}` endpoint, which returns the destination encoded
// as "personal" or "group:<uuid>".
async function selectReuseNotecard(notecardId) {
  const tab = document.querySelector('.tab[data-tab="notecard"]');
  if (tab) tab.click();
  activateSubtab("reuse");
  let scopeValue = "personal";
  try {
    const r = await fetch(`/api/notecards/${notecardId}`);
    if (r.ok) {
      const data = await r.json();
      const dest = data.notecard && data.notecard.destination;
      if (dest && dest.kind === "group" && isUuid(dest.group_id)) {
        scopeValue = `group:${dest.group_id}`;
      } else {
        scopeValue = "personal";
      }
    }
  } catch (_err) {
    // fall back to whatever the scope select already shows
  }
  const scopeSel = $("reuse_scope");
  if (!scopeSel) return;
  // Ensure the scope is present in the dropdown — the groups list may
  // not have loaded yet on a fresh page hit. We retry a few times before
  // giving up.
  for (let i = 0; i < 10; i++) {
    if (Array.from(scopeSel.options).some((o) => o.value === scopeValue)) {
      break;
    }
    await new Promise((r) => setTimeout(r, 50));
  }
  scopeSel.value = scopeValue;
  await loadNotecardsForScope(scopeValue);
  const ncSel = $("reuse_notecard_id");
  if (ncSel) ncSel.value = notecardId;
}

document.addEventListener("DOMContentLoaded", applyPrefillFromQuery);

// =====================================================================
// GLW overlay panel
// =====================================================================

// Toggle the body (via the checkbox) and the rows that depend on the
// active source. The per-source panels themselves are shown/hidden by
// the .glw-tab-panel.active CSS; here we only handle the rows that are
// shown for every source except a specific one (data-glw-source-not).
function refreshGlwPanelVisibility() {
  const enabled = $("glw_enabled").checked;
  const body = $("glw-body");
  if (enabled) body.removeAttribute("hidden");
  else body.setAttribute("hidden", "");
  const source = activeGlwSource();
  for (const el of document.querySelectorAll("[data-glw-source-not]")) {
    el.style.display = el.dataset.glwSourceNot === source ? "none" : "";
  }
}

// Activate a GLW source tab + its panel, mirroring activateSubtab().
// Refreshes the dependent rows and, for the "saved" source, loads the
// saved-GLW dropdown so its options reflect the current scope.
function activateGlwSource(name) {
  document.querySelectorAll(".glw-tab").forEach((t) => {
    t.classList.toggle("active", t.dataset.glwTab === name);
  });
  document.querySelectorAll(".glw-tab-panel").forEach((p) => {
    p.classList.toggle("active", p.id === `glw-source-${name}`);
  });
  refreshGlwPanelVisibility();
  if (name === "saved") loadSavedGlw().catch(() => {});
}

// The currently active GLW source, defaulting to "event_id".
function activeGlwSource() {
  const t = document.querySelector(".glw-tab.active");
  return t ? t.dataset.glwTab : "event_id";
}

// Populate /api/fonts into #glw_font_id. Pre-selects the only entry
// when there is exactly one, leaving the dropdown unchanged otherwise.
async function loadFonts() {
  const sel = $("glw_font_id");
  if (!sel) return;
  try {
    const resp = await fetch("/api/fonts");
    if (!resp.ok) return;
    const { fonts } = await resp.json();
    loadedFonts = fonts;
    sel.replaceChildren();
    for (const f of fonts) {
      const opt = document.createElement("option");
      opt.value = f.id;
      opt.textContent = f.name;
      sel.appendChild(opt);
    }
    if (fonts.length === 1) sel.value = fonts[0].id;
    // refill any per-label font dropdowns now that the list is known
    document
      .querySelectorAll(".label-font")
      .forEach((labelSel) => populateFontSelect(labelSel));
  } catch (err) {
    console.error("font list failed:", err);
  }
}

// Load saved GLW rows for the currently active save_to scope into the
// #glw_saved_id dropdown. Called when the user opens the "saved" tab so
// the most recent options are always reflected.
async function loadSavedGlw() {
  const sel = $("glw_saved_id");
  if (!sel) return;
  const scope = $("save_to").value || "personal";
  try {
    const resp = await fetch(`/api/glw?scope=${encodeURIComponent(scope)}`);
    if (!resp.ok) {
      sel.replaceChildren();
      return;
    }
    const { glw_data } = await resp.json();
    sel.replaceChildren();
    for (const g of glw_data) {
      const opt = document.createElement("option");
      opt.value = g.glw_data_id;
      opt.textContent = g.name;
      sel.appendChild(opt);
    }
    if (glw_data.length === 0) {
      const opt = document.createElement("option");
      opt.value = "";
      opt.textContent = "(none yet — pick another source)";
      sel.appendChild(opt);
    }
  } catch (err) {
    console.error("saved glw list failed:", err);
  }
}

// Maps each GLW style-override field to its colour-swatch input id.
// Shared by the default pre-fill, the override read, and saved-render
// restore so the three stay in lock-step.
const GLW_COLOR_FIELDS = [
  { key: "area_outline_color", id: "glw_area_outline_color" },
  { key: "circle_outline_color", id: "glw_circle_outline_color" },
  { key: "margin_outline_color", id: "glw_margin_outline_color" },
  { key: "wind_color", id: "glw_wind_color" },
  { key: "current_color", id: "glw_current_color" },
  { key: "wave_color", id: "glw_wave_color" },
  { key: "area_fill_color", id: "glw_area_fill_color" },
];

// The renderer's actual style defaults (#rrggbb per field, plus the
// margin-band default), fetched from the server so the form's swatches
// reflect what is really drawn instead of the browser's black default.
// Null until loaded.
let glwStyleDefaults = null;

// Fetch the GLW style defaults and apply them to the form once.
async function loadGlwStyleDefaults() {
  try {
    const resp = await fetch("/api/glw/style-defaults");
    if (!resp.ok) return;
    glwStyleDefaults = await resp.json();
  } catch (err) {
    console.error("glw style defaults failed:", err);
    return;
  }
  applyGlwStyleDefaults();
}

// Pre-fill the colour swatches and the margin-band toggle with the
// renderer's actual defaults. Leaving a swatch at its default makes
// optionalColor() omit that override, so the server applies the full
// default (alpha included).
function applyGlwStyleDefaults() {
  if (!glwStyleDefaults) return;
  const mb = $("glw_margin_band");
  if (mb) mb.checked = !!glwStyleDefaults.margin_band;
  for (const { key, id } of GLW_COLOR_FIELDS) {
    if (key === "area_fill_color") continue;
    const el = $(id);
    const def = glwStyleDefaults[key];
    if (el && def) el.value = def;
  }
  // Area fill has no colour default (the renderer draws no interior
  // fill), so the toggle starts off; seed the picker with the area
  // outline colour for when the user enables it.
  applyGlwAreaFill(glwStyleDefaults.area_fill_color);
}

// Set the area-fill toggle + picker. `color` is a #rrggbb string to
// enable fill with that colour, or null/undefined for "no fill".
function applyGlwAreaFill(color) {
  const toggle = $("glw_area_fill_enabled");
  const picker = $("glw_area_fill_color");
  if (!toggle || !picker) return;
  if (color) {
    toggle.checked = true;
    picker.value = color;
  } else {
    toggle.checked = false;
    if (glwStyleDefaults && glwStyleDefaults.area_outline_color)
      picker.value = glwStyleDefaults.area_outline_color;
  }
  picker.disabled = !toggle.checked;
}

// Serialise the GLW panel into the request shape the server expects.
// Returns null when the panel is disabled (so the caller can omit the
// whole field). Throws Error with a user-friendly message when the user
// has selected a source but left its inputs blank.
function readGlwOptions() {
  if (!$("glw_enabled").checked) return null;
  const source = readGlwSource();
  const fontId = $("glw_font_id").value;
  if (!fontId) {
    throw new Error("Pick a font for the GLW labels.");
  }
  const style = {
    margin_band: $("glw_margin_band").checked,
    area_outline_color: optionalColor(
      "area_outline_color",
      "glw_area_outline_color",
    ),
    circle_outline_color: optionalColor(
      "circle_outline_color",
      "glw_circle_outline_color",
    ),
    margin_outline_color: optionalColor(
      "margin_outline_color",
      "glw_margin_outline_color",
    ),
    wind_color: optionalColor("wind_color", "glw_wind_color"),
    current_color: optionalColor("current_color", "glw_current_color"),
    wave_color: optionalColor("wave_color", "glw_wave_color"),
    // Area fill is a distinct on/off state (no #rrggbb can mean "none"),
    // so the explicit toggle decides whether a fill colour is sent.
    area_fill_color: $("glw_area_fill_enabled").checked
      ? $("glw_area_fill_color").value
      : null,
  };
  const opts = { source, font_id: fontId, style };
  const legendSlot = $("glw_legend_slot");
  if (legendSlot) opts.legend_slot = legendSlot.value;
  if (activeGlwSource() !== "saved") {
    const saveAs = $("glw_save_as").value.trim();
    if (saveAs) opts.save_as = saveAs;
  }
  return opts;
}

function readGlwSource() {
  switch (activeGlwSource()) {
    case "event_id": {
      const raw = $("glw_event_id").value.trim();
      if (!raw) throw new Error("Enter the GLW event id.");
      const event_id = parseInt(raw, 10);
      if (!Number.isFinite(event_id) || event_id < 0) {
        throw new Error("GLW event id must be a non-negative integer.");
      }
      return { type: "event_id", event_id };
    }
    case "event_key": {
      const event_key = $("glw_event_key").value.trim();
      if (!event_key) throw new Error("Enter the GLW event key.");
      return { type: "event_key", event_key };
    }
    case "saved": {
      const glw_data_id = $("glw_saved_id").value.trim();
      if (!glw_data_id) throw new Error("Pick a saved GLW row.");
      return { type: "saved_id", glw_data_id };
    }
    case "pasted": {
      const payload = $("glw_pasted").value.trim();
      if (!payload) throw new Error("Paste the GLW event JSON.");
      return { type: "pasted_json", payload };
    }
    default:
      return null;
  }
}

// Read a `<input type="color">` and return its #rrggbb value as an
// override, or null when it still matches the rendering default for
// `key` — leaving a swatch untouched then sends no override and the
// server applies the full default (alpha included), which a flat
// #rrggbb could not preserve. Before the defaults have loaded we fall
// back to the historical #000000 "unset" sentinel.
function optionalColor(key, id) {
  const el = $(id);
  if (!el) return null;
  const v = el.value;
  if (!v) return null;
  const def = glwStyleDefaults ? glwStyleDefaults[key] : null;
  if (def) return v.toLowerCase() === def.toLowerCase() ? null : v;
  return v === "#000000" ? null : v;
}

function applyGlwSettings(glw) {
  if (!glw) return;
  $("glw_enabled").checked = true;
  refreshGlwPanelVisibility();
  if (glw.font_id) $("glw_font_id").value = glw.font_id;
  if (glw.legend_slot && $("glw_legend_slot"))
    $("glw_legend_slot").value = glw.legend_slot;
  if (glw.save_as) $("glw_save_as").value = glw.save_as;
  // settings_json carries the SavedId carrier exclusively (see the
  // backend rewrite step) so we only ever have to handle this case.
  if (glw.source && glw.source.type === "saved_id") {
    activateGlwSource("saved");
    loadSavedGlw().then(() => {
      const sel = $("glw_saved_id");
      if (sel) sel.value = glw.source.glw_data_id;
    });
  }
  if (glw.style) {
    if ("margin_band" in glw.style)
      $("glw_margin_band").checked = !!glw.style.margin_band;
    for (const { key, id } of GLW_COLOR_FIELDS) {
      if (key === "area_fill_color") continue;
      const el = $(id);
      if (!el) continue;
      // Saved override wins; otherwise fall back to the rendering
      // default so a swatch never shows a stale value from a previous
      // load.
      if (glw.style[key]) el.value = glw.style[key];
      else if (glwStyleDefaults && glwStyleDefaults[key])
        el.value = glwStyleDefaults[key];
    }
    applyGlwAreaFill(glw.style.area_fill_color);
  }
}

document.addEventListener("DOMContentLoaded", () => {
  const enabled = $("glw_enabled");
  if (!enabled) return;
  enabled.addEventListener("change", refreshGlwPanelVisibility);
  document.querySelectorAll(".glw-tab").forEach((tab) => {
    tab.addEventListener("click", () => activateGlwSource(tab.dataset.glwTab));
  });
  $("save_to").addEventListener("change", () => {
    if (activeGlwSource() === "saved") loadSavedGlw().catch(() => {});
    loadLogosForScope().catch(() => {});
  });
  const fillToggle = $("glw_area_fill_enabled");
  if (fillToggle) {
    fillToggle.addEventListener("change", (e) => {
      $("glw_area_fill_color").disabled = !e.target.checked;
    });
  }
  refreshGlwPanelVisibility();
  loadFonts().catch(() => {});
  loadGlwStyleDefaults().catch(() => {});
});

// =====================================================================
// Placement slots & text labels
// =====================================================================

// The nine placement-slot anchors, in 3x3 reading order, with labels.
const SLOT_ANCHORS = [
  "top_left",
  "top_center",
  "top_right",
  "middle_left",
  "center",
  "middle_right",
  "bottom_left",
  "bottom_center",
  "bottom_right",
];
const SLOT_LABELS = {
  top_left: "Top left",
  top_center: "Top centre",
  top_right: "Top right",
  middle_left: "Middle left",
  center: "Centre",
  middle_right: "Middle right",
  bottom_left: "Bottom left",
  bottom_center: "Bottom centre",
  bottom_right: "Bottom right",
};

// Fonts from /api/fonts, shared between the GLW dropdown and the per-label
// dropdowns. Filled by loadFonts().
let loadedFonts = [];

// The most recent placement-slots response, keyed by anchor, or null if it
// has not been computed (or was invalidated by a tab switch). Used to check
// label fit before submitting a render.
let lastPlacementSlots = null;

// Fill a per-label font <select> from loadedFonts, preserving the desired
// selection across reloads via dataset.want.
function populateFontSelect(sel, selected) {
  if (!sel) return;
  if (selected) sel.dataset.want = selected;
  sel.replaceChildren();
  for (const f of loadedFonts) {
    const opt = document.createElement("option");
    opt.value = f.id;
    opt.textContent = f.name;
    sel.appendChild(opt);
  }
  if (sel.dataset.want) sel.value = sel.dataset.want;
  else if (loadedFonts.length === 1) sel.value = loadedFonts[0].id;
}

// The outward default alignment for a slot anchor, matching the server's
// SlotAnchor::default_alignment (top_left -> left/top, center -> centre, ...).
function slotDefaultAlign(anchor) {
  if (anchor === "center") return { h: "center", v: "center" };
  const [vertical, horizontal] = anchor.split("_");
  const v =
    vertical === "top" ? "top" : vertical === "bottom" ? "bottom" : "center";
  const h =
    horizontal === "left"
      ? "left"
      : horizontal === "right"
        ? "right"
        : "center";
  return { h, v };
}

// Whether a label row has no text (all lines blank).
function labelRowBlank(row) {
  return row
    .querySelector(".label-lines")
    .value.split("\n")
    .every((l) => l.trim() === "");
}

// The slot the legend occupies, or null when GLW is off or the legend is
// hidden. Labels may not share this slot.
function legendSlotValue() {
  if (!$("glw_enabled").checked) return null;
  const sel = $("glw_legend_slot");
  if (!sel) return null;
  return sel.value === "none" ? null : sel.value;
}

// Enable/disable both Generate buttons together.
function setGenerateEnabled(ok) {
  for (const id of ["grid_render", "notecard_render"]) {
    const btn = $(id);
    if (btn) btn.disabled = !ok;
  }
}

// Preset a row's alignment dropdowns to its slot's outward default, unless
// the user has already changed them (dataset.touched).
function presetAligns(row) {
  const anchor = row.querySelector(".label-slot").value;
  const def = slotDefaultAlign(anchor);
  const h = row.querySelector(".label-halign");
  const v = row.querySelector(".label-valign");
  if (!h.dataset.touched) h.value = def.h;
  if (!v.dataset.touched) v.value = def.v;
}

// Read the current label rows into the request shape. Blank labels are
// dropped. h_align/v_align are always sent (preset to the slot default).
function readLabels() {
  const out = [];
  for (const row of document.querySelectorAll("#labels-list .label-row")) {
    if (labelRowBlank(row)) continue;
    out.push({
      slot: row.querySelector(".label-slot").value,
      lines: row.querySelector(".label-lines").value.split("\n"),
      font_id: row.querySelector(".label-font").value,
      font_px: parseFloat(row.querySelector(".label-size").value),
      color: row.querySelector(".label-color").value,
      h_align: row.querySelector(".label-halign").value,
      v_align: row.querySelector(".label-valign").value,
    });
  }
  return out;
}

// Measure one label row's rendered text size against the server, cache it on
// the row, then re-run validation.
async function measureLabelRow(row) {
  const lines = row.querySelector(".label-lines").value.split("\n");
  const fontId = row.querySelector(".label-font").value;
  const fontPx = parseFloat(row.querySelector(".label-size").value);
  row.dataset.measured = "";
  const blank = lines.every((l) => l.trim() === "");
  if (blank || !fontId || !Number.isFinite(fontPx) || fontPx <= 0) {
    validateLabels();
    return;
  }
  try {
    const resp = await fetch("/api/text/measure", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({ font_id: fontId, font_px: fontPx, lines }),
    });
    if (!resp.ok) throw new Error(await resp.text());
    const { width, height } = await resp.json();
    row.dataset.mw = String(width);
    row.dataset.mh = String(height);
    row.dataset.measured = "1";
  } catch (err) {
    row.dataset.measured = "";
    row.querySelector(".label-measure").textContent =
      `Could not measure text: ${err.message}`;
  }
  validateLabels();
}

// The full connected free region touching `anchor`, computed by a BFS over
// the per-slot `connected_neighbours` from the last placement-slots response.
// Used to know which slots a `span_fill` placement reserves. Falls back to
// just the anchor when slots have not been computed yet.
function spannedSlots(anchor) {
  if (!lastPlacementSlots) return [anchor];
  const seen = new Set([anchor]);
  const stack = [anchor];
  while (stack.length) {
    const info = lastPlacementSlots[stack.pop()];
    for (const n of (info && info.connected_neighbours) || []) {
      if (!seen.has(n)) {
        seen.add(n);
        stack.push(n);
      }
    }
  }
  return [...seen];
}

// The slots a label row reserves (its anchor, or the whole connected region
// when "fill" is checked).
function labelRowSlots(row) {
  const slot = row.querySelector(".label-slot").value;
  return row.querySelector(".label-span").checked ? spannedSlots(slot) : [slot];
}

// The slots a logo row reserves.
function logoRowSlots(row) {
  const slot = row.querySelector(".logo-slot").value;
  return row.querySelector(".logo-span").checked ? spannedSlots(slot) : [slot];
}

// Validate every label and logo row against one shared pool of placement
// slots, write inline errors next to the offending control, and
// enable/disable the Generate buttons. Mirrors the server-side checks in
// draw_labels_on_map / draw_logos_on_map (including the unified slot
// reservation across labels and logos).
function validateLabels() {
  const labelRows = [...document.querySelectorAll("#labels-list .label-row")];
  const logoRows = [...document.querySelectorAll("#logos-list .logo-row")];
  const legendErr = $("glw_legend_slot_error");
  if (legendErr) legendErr.textContent = "";
  const legendSlot = legendSlotValue();
  let ok = true;

  // Usage count over the shared slot pool from every active placement.
  const slotUse = {};
  for (const row of labelRows) {
    if (labelRowBlank(row)) continue;
    for (const s of labelRowSlots(row)) slotUse[s] = (slotUse[s] || 0) + 1;
  }
  for (const row of logoRows) {
    if (!row.querySelector(".logo-pick").value) continue;
    for (const s of logoRowSlots(row)) slotUse[s] = (slotUse[s] || 0) + 1;
  }

  // ---- text labels ----
  for (const row of labelRows) {
    const slotErr = row.querySelector(".label-slot-error");
    const sizeErr = row.querySelector(".label-size-error");
    const measureEl = row.querySelector(".label-measure");
    slotErr.textContent = "";
    sizeErr.textContent = "";
    measureEl.textContent = "";
    if (labelRowBlank(row)) continue;

    const anchor = row.querySelector(".label-slot").value;
    const slots = labelRowSlots(row);
    if (legendSlot && slots.includes(legendSlot)) {
      slotErr.textContent = "This slot is used by the legend.";
      if (legendErr) legendErr.textContent = "A placement also uses this slot.";
      ok = false;
    }
    if (slots.some((s) => slotUse[s] > 1)) {
      slotErr.textContent = "Another placement already uses this slot.";
      ok = false;
    }

    if (!lastPlacementSlots) {
      measureEl.textContent = 'Run "Find free slots" to check this label fits.';
      ok = false;
      continue;
    }
    const ps = lastPlacementSlots[anchor];
    if (!ps || !ps.available) {
      if (!slotErr.textContent)
        slotErr.textContent = "This slot is covered by route / GLW content.";
      ok = false;
      continue;
    }
    if (row.dataset.measured === "1") {
      const mw = parseInt(row.dataset.mw, 10);
      const mh = parseInt(row.dataset.mh, 10);
      measureEl.textContent = `Text ${mw}×${mh}px — slot free ${ps.free_width}×${ps.free_height}px`;
      if (mw > ps.free_width || mh > ps.free_height) {
        sizeErr.textContent = `Too large for this slot (max ${ps.free_width}×${ps.free_height}px); reduce the font size or text.`;
        ok = false;
      }
    }
  }

  // ---- logos ----
  for (const row of logoRows) {
    const slotErr = row.querySelector(".logo-slot-error");
    const measureEl = row.querySelector(".logo-measure");
    slotErr.textContent = "";
    measureEl.textContent = "";
    const pick = row.querySelector(".logo-pick");
    if (!pick.value) {
      measureEl.textContent = "Choose a logo.";
      ok = false;
      continue;
    }

    const anchor = row.querySelector(".logo-slot").value;
    const slots = logoRowSlots(row);
    if (legendSlot && slots.includes(legendSlot)) {
      slotErr.textContent = "This slot is used by the legend.";
      if (legendErr) legendErr.textContent = "A placement also uses this slot.";
      ok = false;
    }
    if (slots.some((s) => slotUse[s] > 1)) {
      slotErr.textContent = "Another placement already uses this slot.";
      ok = false;
    }

    const scale = parseInt(row.querySelector(".logo-scale").value, 10) || 1;
    const opt = pick.selectedOptions[0];
    const w = (opt ? parseInt(opt.dataset.w, 10) || 0 : 0) * scale;
    const h = (opt ? parseInt(opt.dataset.h, 10) || 0 : 0) * scale;

    if (!lastPlacementSlots) {
      measureEl.textContent = 'Run "Find free slots" to check this logo fits.';
      ok = false;
      continue;
    }
    const ps = lastPlacementSlots[anchor];
    if (!ps || !ps.available) {
      if (!slotErr.textContent)
        slotErr.textContent = "This slot is covered by route / GLW content.";
      ok = false;
      continue;
    }
    measureEl.textContent = `Logo ${w}×${h}px — slot free ${ps.free_width}×${ps.free_height}px`;
    if (w > ps.free_width || h > ps.free_height) {
      measureEl.textContent +=
        " — too large; enable Fill connected free area or choose a larger slot.";
      ok = false;
    }
  }

  setGenerateEnabled(ok);
}

// Restore a label row's controls from a saved TextLabel (regenerate).
function applyLabelPreset(row, l) {
  if (Array.isArray(l.lines))
    row.querySelector(".label-lines").value = l.lines.join("\n");
  if (l.font_id)
    populateFontSelect(row.querySelector(".label-font"), l.font_id);
  if (l.font_px != null) row.querySelector(".label-size").value = l.font_px;
  if (l.color) row.querySelector(".label-color").value = l.color;
  if (l.slot) row.querySelector(".label-slot").value = l.slot;
  const h = row.querySelector(".label-halign");
  const v = row.querySelector(".label-valign");
  if (l.h_align) {
    h.value = l.h_align;
    h.dataset.touched = "1";
  }
  if (l.v_align) {
    v.value = l.v_align;
    v.dataset.touched = "1";
  }
}

// Add a new label row (optionally pre-filled from a saved TextLabel) and
// wire up its listeners.
function addLabelRow(preset) {
  const tpl = $("label-row-template");
  if (!tpl) return null;
  const row = tpl.content.firstElementChild.cloneNode(true);

  const slotSel = row.querySelector(".label-slot");
  for (const a of SLOT_ANCHORS) {
    const opt = document.createElement("option");
    opt.value = a;
    opt.textContent = SLOT_LABELS[a];
    slotSel.appendChild(opt);
  }
  populateFontSelect(row.querySelector(".label-font"));

  const measure = debounce(() => measureLabelRow(row), 300);
  row.querySelector(".label-lines").addEventListener("input", measure);
  row.querySelector(".label-size").addEventListener("input", measure);
  row.querySelector(".label-font").addEventListener("change", measure);
  slotSel.addEventListener("change", () => {
    presetAligns(row);
    validateLabels();
  });
  const h = row.querySelector(".label-halign");
  const v = row.querySelector(".label-valign");
  h.addEventListener("change", () => {
    h.dataset.touched = "1";
    validateLabels();
  });
  v.addEventListener("change", () => {
    v.dataset.touched = "1";
    validateLabels();
  });
  row.querySelector(".label-remove").addEventListener("click", () => {
    row.remove();
    validateLabels();
  });

  $("labels-list").appendChild(row);
  if (preset) applyLabelPreset(row, preset);
  presetAligns(row);
  measureLabelRow(row);
  return row;
}

// Replace all label rows from saved settings (regenerate).
function applyLabels(labels) {
  const list = $("labels-list");
  if (list) list.replaceChildren();
  if (Array.isArray(labels)) {
    for (const l of labels) addLabelRow(l);
  }
  validateLabels();
}

// Logos available in the current save_to scope, shared between the per-row
// pickers. Filled by loadLogosForScope().
let loadedLogos = [];

// Fill a per-row logo <select> from loadedLogos, preserving the desired
// selection across reloads via dataset.want. Each option carries the logo's
// intrinsic pixel size in data-w / data-h for the fit check.
function populateLogoSelect(sel, selected) {
  if (!sel) return;
  if (selected) sel.dataset.want = selected;
  const want = sel.dataset.want || sel.value;
  sel.replaceChildren();
  const placeholder = document.createElement("option");
  placeholder.value = "";
  placeholder.textContent = loadedLogos.length
    ? "(choose a logo)"
    : "(no logos in this scope)";
  sel.appendChild(placeholder);
  for (const l of loadedLogos) {
    const opt = document.createElement("option");
    opt.value = l.logo_id;
    opt.textContent = `${l.name} (${l.width}×${l.height})`;
    opt.dataset.w = String(l.width);
    opt.dataset.h = String(l.height);
    sel.appendChild(opt);
  }
  if (want && loadedLogos.some((l) => l.logo_id === want)) sel.value = want;
}

// Load the logos for the active save_to scope and repopulate every row's
// picker. Logos must live in the same library as the render (same-scope
// rule), so this re-runs whenever save_to changes.
async function loadLogosForScope() {
  const scope = $("save_to") ? $("save_to").value || "personal" : "personal";
  try {
    const resp = await fetch(`/api/logos?scope=${encodeURIComponent(scope)}`);
    loadedLogos = resp.ok ? (await resp.json()).logos || [] : [];
  } catch (_err) {
    loadedLogos = [];
  }
  document
    .querySelectorAll("#logos-list .logo-row")
    .forEach((row) => populateLogoSelect(row.querySelector(".logo-pick")));
  validateLabels();
}

// Read the current logo rows into the request shape. Rows with no logo
// chosen are dropped.
function readLogos() {
  const out = [];
  for (const row of document.querySelectorAll("#logos-list .logo-row")) {
    const logoId = row.querySelector(".logo-pick").value;
    if (!logoId) continue;
    out.push({
      slot: row.querySelector(".logo-slot").value,
      logo_id: logoId,
      scale: parseInt(row.querySelector(".logo-scale").value, 10) || 1,
      span_fill: row.querySelector(".logo-span").checked,
      h_align: row.querySelector(".logo-halign").value,
      v_align: row.querySelector(".logo-valign").value,
    });
  }
  return out;
}

// Preset a logo row's alignment dropdowns to its slot's outward default,
// unless the user has already changed them.
function presetLogoAligns(row) {
  const def = slotDefaultAlign(row.querySelector(".logo-slot").value);
  const h = row.querySelector(".logo-halign");
  const v = row.querySelector(".logo-valign");
  if (!h.dataset.touched) h.value = def.h;
  if (!v.dataset.touched) v.value = def.v;
}

// Update a logo row's preview thumbnail to the chosen logo.
function updateLogoPreview(row) {
  const pick = row.querySelector(".logo-pick");
  const img = row.querySelector(".logo-preview");
  if (pick.value) {
    img.src = `/api/logos/${pick.value}/image`;
    img.classList.remove("hidden");
  } else {
    img.removeAttribute("src");
    img.classList.add("hidden");
  }
}

// Restore a logo row's controls from a saved LogoPlacement (regenerate).
function applyLogoPreset(row, l) {
  if (l.logo_id) populateLogoSelect(row.querySelector(".logo-pick"), l.logo_id);
  if (l.slot) row.querySelector(".logo-slot").value = l.slot;
  if (l.scale != null) row.querySelector(".logo-scale").value = String(l.scale);
  if (l.span_fill) row.querySelector(".logo-span").checked = true;
  const h = row.querySelector(".logo-halign");
  const v = row.querySelector(".logo-valign");
  if (l.h_align) {
    h.value = l.h_align;
    h.dataset.touched = "1";
  }
  if (l.v_align) {
    v.value = l.v_align;
    v.dataset.touched = "1";
  }
}

// Add a new logo row (optionally pre-filled) and wire up its listeners.
function addLogoRow(preset) {
  const tpl = $("logo-row-template");
  if (!tpl) return null;
  const row = tpl.content.firstElementChild.cloneNode(true);

  const slotSel = row.querySelector(".logo-slot");
  for (const a of SLOT_ANCHORS) {
    const opt = document.createElement("option");
    opt.value = a;
    opt.textContent = SLOT_LABELS[a];
    slotSel.appendChild(opt);
  }
  populateLogoSelect(row.querySelector(".logo-pick"));

  const pick = row.querySelector(".logo-pick");
  pick.addEventListener("change", () => {
    updateLogoPreview(row);
    validateLabels();
  });
  slotSel.addEventListener("change", () => {
    presetLogoAligns(row);
    validateLabels();
  });
  row.querySelector(".logo-scale").addEventListener("change", validateLabels);
  row.querySelector(".logo-span").addEventListener("change", validateLabels);
  const h = row.querySelector(".logo-halign");
  const v = row.querySelector(".logo-valign");
  h.addEventListener("change", () => {
    h.dataset.touched = "1";
    validateLabels();
  });
  v.addEventListener("change", () => {
    v.dataset.touched = "1";
    validateLabels();
  });
  row.querySelector(".logo-remove").addEventListener("click", () => {
    row.remove();
    validateLabels();
  });

  $("logos-list").appendChild(row);
  if (preset) applyLogoPreset(row, preset);
  presetLogoAligns(row);
  updateLogoPreview(row);
  return row;
}

// Replace all logo rows from saved settings (regenerate).
function applyLogos(logos) {
  const list = $("logos-list");
  if (list) list.replaceChildren();
  if (Array.isArray(logos)) {
    for (const l of logos) addLogoRow(l);
  }
  validateLabels();
}

// Render the 3x3 availability grid from a placement-slots response.
function renderSlotGrid(data) {
  const grid = $("placement-grid");
  if (!grid) return;
  grid.replaceChildren();
  const byAnchor = {};
  for (const s of data.slots) byAnchor[s.slot] = s;
  for (const a of SLOT_ANCHORS) {
    const s = byAnchor[a] || {
      available: false,
      free_width: 0,
      free_height: 0,
    };
    const cell = document.createElement("div");
    cell.className = `placement-cell ${s.available ? "free" : "occupied"}`;
    const title = document.createElement("strong");
    title.textContent = SLOT_LABELS[a];
    const info = document.createElement("span");
    info.textContent = s.available
      ? `${s.free_width}×${s.free_height}px free`
      : "occupied";
    cell.append(title, info);
    grid.appendChild(cell);
  }
}

// Compute free placement slots for the active tab (grid or notecard),
// mirroring the corresponding render request, then refresh the grid and
// re-validate the labels.
async function findFreeSlots() {
  const statusEl = $("placement-status");
  statusEl.textContent = "Computing free slots…";
  try {
    const activeTab = document.querySelector(".tab.active");
    const which = activeTab ? activeTab.dataset.tab : "grid";
    let resp;
    if (which === "grid") {
      const glw = readGlwOptions();
      const body = {
        lower_left_x: parseInt($("ll_x").value, 10),
        lower_left_y: parseInt($("ll_y").value, 10),
        upper_right_x: parseInt($("ur_x").value, 10),
        upper_right_y: parseInt($("ur_y").value, 10),
        ...readSharedParams(),
      };
      if (glw) body.glw = glw;
      resp = await fetch("/api/render/placement-slots/grid-rectangle", {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify(body),
      });
    } else {
      const fd = new FormData();
      appendNotecardSourceToForm(fd);
      appendBordersToForm(fd);
      const shared = readSharedParams();
      fd.append("max_width", String(shared.max_width));
      fd.append("max_height", String(shared.max_height));
      fd.append("format", shared.format);
      if (shared.missing_map_tile_color)
        fd.append("missing_map_tile_color", shared.missing_map_tile_color);
      if (shared.missing_region_color)
        fd.append("missing_region_color", shared.missing_region_color);
      fd.append("color", $("route_color").value);
      const glw = readGlwOptions();
      if (glw) fd.append("glw_json", JSON.stringify(glw));
      resp = await fetch("/api/render/placement-slots/usb-notecard", {
        method: "POST",
        body: fd,
      });
    }
    if (!resp.ok) throw new Error(await resp.text());
    const data = await resp.json();
    lastPlacementSlots = {};
    for (const s of data.slots) lastPlacementSlots[s.slot] = s;
    renderSlotGrid(data);
    statusEl.textContent = `Free slots for a ${data.image_width}×${data.image_height}px render.`;
    document
      .querySelectorAll("#labels-list .label-row")
      .forEach((row) => measureLabelRow(row));
    validateLabels();
  } catch (err) {
    statusEl.textContent = `Could not compute slots: ${err.message}`;
  }
}

// Small debounce helper for the live text measurement.
function debounce(fn, ms) {
  let timer;
  return (...args) => {
    clearTimeout(timer);
    timer = setTimeout(() => fn(...args), ms);
  };
}

document.addEventListener("DOMContentLoaded", () => {
  const addBtn = $("add_label");
  if (addBtn)
    addBtn.addEventListener("click", () => {
      addLabelRow();
      validateLabels();
    });
  const addLogoBtn = $("add_logo");
  if (addLogoBtn)
    addLogoBtn.addEventListener("click", () => {
      addLogoRow();
      validateLabels();
    });
  loadLogosForScope().catch(() => {});
  const findBtn = $("find_slots");
  if (findBtn) findBtn.addEventListener("click", () => findFreeSlots());
  const legendSel = $("glw_legend_slot");
  if (legendSel) legendSel.addEventListener("change", validateLabels);
  const glwEnabled = $("glw_enabled");
  if (glwEnabled) glwEnabled.addEventListener("change", validateLabels);
  // The slot occupancy differs between the grid and notecard tabs, so a tab
  // switch invalidates the cached result.
  document.querySelectorAll(".tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      lastPlacementSlots = null;
      const grid = $("placement-grid");
      if (grid) grid.replaceChildren();
      const st = $("placement-status");
      if (st) st.textContent = "";
      validateLabels();
    });
  });
  validateLabels();
});
