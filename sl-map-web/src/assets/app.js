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

// Which source tab ("grid" or "notecard") is active. The shared Preview and
// Generate buttons dispatch on this.
function activeTab() {
  const t = document.querySelector(".tab.active");
  return t ? t.dataset.tab : "grid";
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

// Fetch the GLW overlay for `rect` rendered at preview zoom `z` as a blob
// URL, or null when the GLW panel is disabled. Reuses the same `readGlwOptions`
// payload the real render submits, so the preview overlay is drawn by the very
// same server-side code path. Throws (with the panel's validation message, or
// the server's error text) so the caller can surface it.
async function fetchGlwOverlay(rect, z) {
  const glw = readGlwOptions();
  if (!glw) return null;
  const resp = await fetch("/api/render/glw-preview", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      lower_left_x: rect.lower_left_x,
      lower_left_y: rect.lower_left_y,
      upper_right_x: rect.upper_right_x,
      upper_right_y: rect.upper_right_y,
      zoom: z,
      glw,
    }),
  });
  if (!resp.ok) throw new Error(await resp.text());
  return URL.createObjectURL(await resp.blob());
}

// Fetch the GLW base legend rendered at the final-image resolution as a blob
// URL, or null when GLW is disabled or no legend slot is chosen. The server
// draws only the legend, at the exact slot and size the final render uses,
// onto a transparent image the size of the final image; the caller drops it
// into the bounds rectangle so it lines up. Throws on server error.
async function fetchGlwLegendOverlay(rect) {
  const glw = readGlwOptions();
  if (!glw) return null;
  if (!glw.legend_slot || glw.legend_slot === "none") return null;
  const shared = readSharedParams();
  const resp = await fetch("/api/render/glw-legend-preview", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      lower_left_x: rect.lower_left_x,
      lower_left_y: rect.lower_left_y,
      upper_right_x: rect.upper_right_x,
      upper_right_y: rect.upper_right_y,
      max_width: shared.max_width,
      max_height: shared.max_height,
      glw,
    }),
  });
  if (!resp.ok) throw new Error(await resp.text());
  return URL.createObjectURL(await resp.blob());
}

// Fetch the route rendered at the final-image resolution as a blob URL, or null
// when there are fewer than two waypoints (nothing to draw). The server draws
// the route with the very same spline + arrows code the final render uses, onto
// a transparent image the size of the final image; the caller drops it into the
// bounds rectangle so it lines up with the tiles and looks identical to the
// output. Throws on server error.
async function fetchRoutePreview(rect, waypoints) {
  if (!waypoints || waypoints.length <= 1) return null;
  const shared = readSharedParams();
  const resp = await fetch("/api/render/route-preview", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({
      lower_left_x: rect.lower_left_x,
      lower_left_y: rect.lower_left_y,
      upper_right_x: rect.upper_right_x,
      upper_right_y: rect.upper_right_y,
      max_width: shared.max_width,
      max_height: shared.max_height,
      color: $("route_color").value,
      waypoints: waypoints.map((w) => ({
        region_x: w.region_x,
        region_y: w.region_y,
        x: w.x,
        y: w.y,
      })),
    }),
  });
  if (!resp.ok) throw new Error(await resp.text());
  return URL.createObjectURL(await resp.blob());
}

// Fetch the text labels + logos rendered at the final-image resolution as a
// blob URL, or null when there are none. The server places them on an
// overlay-only map exactly as the final render does, so they line up with the
// preview once dropped into the bounds rectangle. Mirrors the render submit:
// the grid tab posts JSON, the notecard tab posts the multipart form (the
// server re-derives the rectangle from the notecard). Throws on server error.
async function fetchPlacementOverlay(rect) {
  // Only the placements that currently fit: the preview stays useful even when
  // one label/logo overflows its slot (it is shown as a red box instead).
  const labels = readLabels(false);
  const logos = readLogos(false);
  if (!labels.length && !logos.length) return null;
  const activeTab = document.querySelector(".tab.active");
  const which = activeTab ? activeTab.dataset.tab : "grid";
  let resp;
  if (which === "grid") {
    const glw = readGlwOptions();
    const body = {
      lower_left_x: rect.lower_left_x,
      lower_left_y: rect.lower_left_y,
      upper_right_x: rect.upper_right_x,
      upper_right_y: rect.upper_right_y,
      ...readSharedParams(),
      labels,
      logos,
    };
    if (glw) body.glw = glw;
    resp = await fetch("/api/render/placement-preview/grid-rectangle", {
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
    fd.append("labels_json", JSON.stringify(labels));
    fd.append("logos_json", JSON.stringify(logos));
    resp = await fetch("/api/render/placement-preview/usb-notecard", {
      method: "POST",
      body: fd,
    });
  }
  if (!resp.ok) throw new Error(await resp.text());
  return URL.createObjectURL(await resp.blob());
}

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

  viewport.appendChild(svg);

  // Route overlay. The server rasterises just the route — the very same
  // Catmull-Rom spline + per-waypoint arrows + route colour the final render
  // draws — onto a transparent image at the final-image resolution, which we
  // drop into the bounds rectangle (the browser scales it down) so the preview
  // route is a pixel-faithful copy of the output. Inserted before the bounds
  // SVG so the dashed guide stays on top; the GLW overlay (below) is inserted
  // before this image so the route stays above the GLW shapes, matching the
  // final render's layering. The fetch is async, so the placeholder <img> is
  // positioned now and its src filled in on arrival.
  let routeImg = null;
  if (waypoints && waypoints.length > 1) {
    routeImg = document.createElement("img");
    routeImg.className = "route-overlay";
    routeImg.style.left = `${boundsX.toFixed(1)}px`;
    routeImg.style.top = `${boundsY.toFixed(1)}px`;
    routeImg.style.width = `${boundsW.toFixed(1)}px`;
    routeImg.style.height = `${boundsH.toFixed(1)}px`;
    viewport.insertBefore(routeImg, svg);
    fetchRoutePreview(rect, waypoints)
      .then((url) => {
        if (!url) {
          routeImg.remove();
          return;
        }
        routeImg.src = url;
        routeImg.addEventListener("load", () => URL.revokeObjectURL(url), {
          once: true,
        });
      })
      .catch((err) => {
        routeImg.remove();
        $("preview-status").textContent =
          `Route overlay failed: ${err.message}`;
      });
  }

  // GLW overlay. The server rasterises just the geographic GLW shapes and
  // their labels (the legend is excluded — it is placed separately by the
  // placement-slot logic) onto a transparent image the size of the final-image
  // bounds at this same zoom level, which we drop into the bounds rectangle so
  // it lines up with the tiles. Inserted before the route image so the route
  // stays on top (matching the final render's layering: GLW under the route).
  // The fetch is async, so the placeholder <img> is positioned now and its src
  // filled in on arrival.
  if ($("glw_enabled") && $("glw_enabled").checked) {
    const glwImg = document.createElement("img");
    glwImg.className = "glw-overlay";
    glwImg.style.left = `${boundsX.toFixed(1)}px`;
    glwImg.style.top = `${boundsY.toFixed(1)}px`;
    glwImg.style.width = `${boundsW.toFixed(1)}px`;
    glwImg.style.height = `${boundsH.toFixed(1)}px`;
    viewport.insertBefore(glwImg, routeImg || svg);
    fetchGlwOverlay(rect, z)
      .then((url) => {
        if (!url) {
          glwImg.remove();
          return;
        }
        glwImg.src = url;
        glwImg.addEventListener("load", () => URL.revokeObjectURL(url), {
          once: true,
        });
      })
      .catch((err) => {
        glwImg.remove();
        $("preview-status").textContent = `GLW overlay failed: ${err.message}`;
      });
  }

  // Record the bounds rectangle so the overlays can position content inside it
  // without re-deriving the geometry. Set before drawing the overlays because
  // they (and later refreshes) read it back from the dataset.
  viewport.dataset.boundsX = String(boundsX);
  viewport.dataset.boundsY = String(boundsY);
  viewport.dataset.boundsW = String(boundsW);
  viewport.dataset.boundsH = String(boundsH);

  // GLW legend (independent of fit). The labels/logos overlay is drawn by
  // findFreeSlots() below, once the per-slot fit has been recomputed, so an
  // overflowing placement is excluded rather than failing the whole batch.
  drawLegendOverlay(viewport, rect);

  container.appendChild(viewport);
  fitViewport(container, viewport, widthPx, heightPx);
  drawSlotsOverlay(viewport);
  lastPreviewRect = rect;

  $("preview-status").textContent =
    `Preview at zoom ${z} (${pixelsPerRegion(z)} px/region) — ` +
    `${tilesX * tilesY} tile${tilesX * tilesY === 1 ? "" : "s"}, ` +
    `${widthPx}×${heightPx} px.`;

  // Auto-compute the free slots so the per-slot buttons appear with the tiles.
  findFreeSlots();
}

// Draw (or refresh) the GLW base legend overlay on a preview viewport, reading
// the bounds rectangle from the viewport dataset. Shown only when GLW is
// enabled; the fetch returns null (and the image is dropped) when no legend
// slot is set.
// Clear a stale overlay error from the preview status, but only if it is still
// showing that error (so a successful overlay refresh clears its own prior
// failure without clobbering the "Preview at zoom…" line or another overlay's
// error).
function clearOverlayError(prefix) {
  const el = $("preview-status");
  if (el && el.textContent.startsWith(prefix)) el.textContent = "";
}

function drawLegendOverlay(viewport, rect) {
  if (!viewport) return;
  const old = viewport.querySelector("img.glw-legend");
  if (old) old.remove();
  if (!($("glw_enabled") && $("glw_enabled").checked)) {
    clearOverlayError("GLW legend failed");
    return;
  }
  const bx = parseFloat(viewport.dataset.boundsX);
  const by = parseFloat(viewport.dataset.boundsY);
  const bw = parseFloat(viewport.dataset.boundsW);
  const bh = parseFloat(viewport.dataset.boundsH);
  if (![bx, by, bw, bh].every(Number.isFinite)) return;
  const img = document.createElement("img");
  img.className = "glw-legend";
  img.style.left = `${bx.toFixed(1)}px`;
  img.style.top = `${by.toFixed(1)}px`;
  img.style.width = `${bw.toFixed(1)}px`;
  img.style.height = `${bh.toFixed(1)}px`;
  viewport.appendChild(img);
  fetchGlwLegendOverlay(rect)
    .then((url) => {
      clearOverlayError("GLW legend failed");
      if (!url) {
        img.remove();
        return;
      }
      img.src = url;
      img.addEventListener("load", () => URL.revokeObjectURL(url), {
        once: true,
      });
    })
    .catch((err) => {
      img.remove();
      $("preview-status").textContent = `GLW legend failed: ${err.message}`;
    });
}

// Draw (or refresh) the text-labels/logos overlay on a preview viewport. The
// fetch returns null (image dropped) when there are no labels or logos.
function drawPlacementOverlay(viewport, rect) {
  if (!viewport) return;
  const old = viewport.querySelector("img.placement-overlay");
  if (old) old.remove();
  const bx = parseFloat(viewport.dataset.boundsX);
  const by = parseFloat(viewport.dataset.boundsY);
  const bw = parseFloat(viewport.dataset.boundsW);
  const bh = parseFloat(viewport.dataset.boundsH);
  if (![bx, by, bw, bh].every(Number.isFinite)) return;
  const img = document.createElement("img");
  img.className = "placement-overlay";
  img.style.left = `${bx.toFixed(1)}px`;
  img.style.top = `${by.toFixed(1)}px`;
  img.style.width = `${bw.toFixed(1)}px`;
  img.style.height = `${bh.toFixed(1)}px`;
  viewport.appendChild(img);
  fetchPlacementOverlay(rect)
    .then((url) => {
      clearOverlayError("Placement preview failed");
      if (!url) {
        img.remove();
        return;
      }
      img.src = url;
      img.addEventListener("load", () => URL.revokeObjectURL(url), {
        once: true,
      });
    })
    .catch((err) => {
      img.remove();
      $("preview-status").textContent =
        `Placement preview failed: ${err.message}`;
    });
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

function previewGrid() {
  const rect = {
    lower_left_x: parseInt($("ll_x").value, 10),
    lower_left_y: parseInt($("ll_y").value, 10),
    upper_right_x: parseInt($("ur_x").value, 10),
    upper_right_y: parseInt($("ur_y").value, 10),
  };
  renderPreview(rect, null);
}

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

async function previewNotecard() {
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
}

// Shared Preview button (in the Preview panel): dispatch on the active tab.
$("preview_btn").addEventListener("click", () => {
  if (activeTab() === "notecard") previewNotecard();
  else previewGrid();
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

async function renderGrid() {
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
}

async function renderNotecard() {
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
}

// Shared Generate button (in the Render panel): dispatch on the active tab.
$("generate_btn").addEventListener("click", () => {
  if (activeTab() === "notecard") renderNotecard();
  else renderGrid();
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
  // Rebuild placement state from scratch so restored labels/logos/legend
  // replace whatever was configured before.
  initSlotGroups();
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
  { key: "label_color", id: "glw_label_color" },
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
    const el = $(id);
    const def = glwStyleDefaults[key];
    if (el && def) el.value = def;
  }
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
    label_color: optionalColor("label_color", "glw_label_color"),
  };
  const opts = { source, font_id: fontId, style };
  // The legend's slot is chosen on the preview overlay (the legend button).
  opts.legend_slot = legendSlotFromState();
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
  if (glw.legend_slot && glw.legend_slot !== "none") {
    const g = groupOf(glw.legend_slot);
    if (g) {
      g.type = "legend";
      g.config = null;
    }
  }
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
      const el = $(id);
      if (!el) continue;
      // Saved override wins; otherwise fall back to the rendering
      // default so a swatch never shows a stale value from a previous
      // load.
      if (glw.style[key]) el.value = glw.style[key];
      else if (glwStyleDefaults && glwStyleDefaults[key])
        el.value = glwStyleDefaults[key];
    }
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
// Each slot's [column, row] within the 3x3 division of the final-image bounds,
// used to place the marker for an occupied slot at its nominal cell centre.
const SLOT_CELL = {
  top_left: [0, 0],
  top_center: [1, 0],
  top_right: [2, 0],
  middle_left: [0, 1],
  center: [1, 1],
  middle_right: [2, 1],
  bottom_left: [0, 2],
  bottom_center: [1, 2],
  bottom_right: [2, 2],
};

// Fonts from /api/fonts, shared between the GLW dropdown and the per-label
// dropdowns. Filled by loadFonts().
let loadedFonts = [];

// The most recent placement-slots response, keyed by anchor, or null if it
// has not been computed (or was invalidated by a tab switch). Used to check
// fit and to draw the preview overlay.
let lastPlacementSlots = null;
// Combined-rectangle info for the requested multi-slot groups, keyed by the
// group's sorted slot-name list joined with ",". Filled by findFreeSlots().
let lastGroupRects = {};
// Pixel size of the final image the slot rectangles are measured in, so the
// preview overlay can scale them into the bounds rectangle. Null until the
// slots have been computed.
let lastPlacementImageSize = null;
// The rectangle the current preview was rendered for, so placement changes can
// refresh the legend / labels / logos overlays without a full re-render.
let lastPreviewRect = null;

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

// Enable/disable the Generate button.
function setGenerateEnabled(ok) {
  const btn = $("generate_btn");
  if (btn) btn.disabled = !ok;
}

// --- per-slot placement state -------------------------------------------
//
// Every placement (text label, logo or GLW legend) is attached to a *group*
// of one or more combined slot anchors. The nine slots start as singleton
// groups with type "none". Combining merges two groups; splitting breaks a
// group back into singletons. Each group holds at most one element.
//   group = { id, slots:[anchor…], type:"none"|"label"|"logo"|"legend",
//             config, error }
let slotGroups = [];
let slotToGroup = new Map();
let nextGroupId = 1;

// Reset to nine singleton "none" groups.
function initSlotGroups() {
  slotGroups = [];
  slotToGroup = new Map();
  nextGroupId = 1;
  for (const a of SLOT_ANCHORS) addGroup([a], "none", null);
}

// Slot anchors in canonical reading order, de-duplicated.
function sortAnchors(slots) {
  return [...new Set(slots)].sort(
    (a, b) => SLOT_ANCHORS.indexOf(a) - SLOT_ANCHORS.indexOf(b),
  );
}

function addGroup(slots, type, config) {
  const g = {
    id: nextGroupId++,
    slots: sortAnchors(slots),
    type,
    config,
    error: null,
  };
  slotGroups.push(g);
  for (const a of g.slots) slotToGroup.set(a, g);
  return g;
}

function removeGroupObj(g) {
  slotGroups = slotGroups.filter((x) => x !== g);
  for (const a of g.slots) if (slotToGroup.get(a) === g) slotToGroup.delete(a);
}

function groupOf(anchor) {
  return slotToGroup.get(anchor) || null;
}

// The primary anchor (top-left-most slot) of a group, used for alignment
// defaults and as the legend / label / logo `slot` sent to the server.
function primaryAnchor(g) {
  return g.slots[0];
}

// Human label for a group, e.g. "Top left" or "Top left + Top centre".
function groupName(g) {
  return g.slots.map((a) => SLOT_LABELS[a]).join(" + ");
}

// Short description of a group's element, for the combine choice modal.
function describe(g) {
  return g.type === "label"
    ? "the text label"
    : g.type === "logo"
      ? "the logo"
      : g.type === "legend"
        ? "the GLW legend"
        : "nothing";
}

// The slot the GLW legend occupies, or "none" when no group holds it. Sent as
// readGlwOptions().legend_slot.
function legendSlotFromState() {
  const g = slotGroups.find((x) => x.type === "legend");
  return g ? primaryAnchor(g) : "none";
}

// Assign an element to a group (clearing any existing legend elsewhere when
// the new element is the legend), then re-validate and refresh the preview.
function assignGroup(g, type, config) {
  if (type === "legend") {
    for (const o of slotGroups)
      if (o !== g && o.type === "legend") {
        o.type = "none";
        o.config = null;
      }
  }
  g.type = type;
  g.config = config;
  refreshPlacement();
}

function setLegend(g) {
  assignGroup(g, "legend", null);
}

async function clearGroup(g) {
  if (g.type === "none") return;
  g.type = "none";
  g.config = null;
  refreshPlacement();
}

// Merge whatever groups currently cover `slots` into one fresh group (used
// when restoring saved combined placements).
function ensureGroupForSlots(slots) {
  const set = sortAnchors(slots);
  for (const a of set) {
    const g = groupOf(a);
    if (g) removeGroupObj(g);
  }
  return addGroup(set, "none", null);
}

// Re-validate and refresh the preview overlays after a placement change.
function refreshPlacement() {
  validateLabels();
  refreshPlacementPreview();
}

// Re-draw the legend and label/logo overlays on the current preview to match
// the placement state, without a full re-render.
function refreshPlacementPreview() {
  const vp = $("preview-container").querySelector(".viewport");
  if (!vp || !lastPreviewRect) return;
  drawLegendOverlay(vp, lastPreviewRect);
  drawPlacementOverlay(vp, lastPreviewRect);
}

// Measure rendered text size against the server. Returns {width, height}.
async function measureText(fontId, fontPx, lines) {
  const resp = await fetch("/api/text/measure", {
    method: "POST",
    headers: { "content-type": "application/json" },
    body: JSON.stringify({ font_id: fontId, font_px: fontPx, lines }),
  });
  if (!resp.ok) throw new Error(await resp.text());
  return resp.json();
}

// Re-measure every label group's text (size is independent of the slots, so
// this only runs after a restore or a label edit), caching mw/mh on the
// config, then validate.
async function remeasureAllLabels() {
  for (const g of slotGroups) {
    if (g.type !== "label") continue;
    try {
      const m = await measureText(
        g.config.font_id,
        g.config.font_px,
        g.config.lines,
      );
      g.config.mw = m.width;
      g.config.mh = m.height;
    } catch (_err) {
      // leave mw/mh as-is; validation treats missing measure as "unknown"
    }
  }
  validateLabels();
}

// Stable key for a group's slot set (matches the server's GroupDto.slots order).
function groupKey(slots) {
  return slots.join(",");
}

// The free rectangle + size available to a group, from the last placement-slots
// response: a single slot uses its own free_rect, a combined group its reported
// group rect. Returns {available, free_rect, free_width, free_height} or null.
function rectForGroup(g) {
  if (!lastPlacementSlots) return null;
  if (g.slots.length === 1) {
    const s = lastPlacementSlots[g.slots[0]];
    if (!s) return null;
    return {
      available: s.available,
      free_rect: s.free_rect || null,
      free_width: s.free_width,
      free_height: s.free_height,
    };
  }
  const gr = lastGroupRects[groupKey(g.slots)];
  if (!gr) return null;
  return {
    available: gr.available,
    free_rect: gr.free_rect || null,
    free_width: gr.free_width,
    free_height: gr.free_height,
  };
}

// Read the label groups into the render request shape. With
// `includeErrored = false` the groups that currently fail validation (e.g. text
// too large for the slot) are skipped, so a single overflowing label cannot
// fail the whole preview batch.
function readLabels(includeErrored = true) {
  const out = [];
  for (const g of slotGroups) {
    if (g.type !== "label") continue;
    if (!includeErrored && g.error) continue;
    const c = g.config;
    out.push({
      slot: primaryAnchor(g),
      slots: g.slots.slice(),
      lines: c.lines,
      font_id: c.font_id,
      font_px: c.font_px,
      color: c.color,
      h_align: c.h_align,
      v_align: c.v_align,
    });
  }
  return out;
}

// Validate every label and logo row against one shared pool of placement
// slots, write inline errors next to the offending control, and
// enable/disable the Generate buttons. Mirrors the server-side checks in
// draw_labels_on_map / draw_logos_on_map (including the unified slot
// reservation across labels and logos).
// Validate every placement group against its (combined) free rectangle. Sets
// each group's `error` (shown as a red overlay box), enables/disables Generate,
// and redraws the slot overlay. The slot pool is conflict-free by construction
// (each slot belongs to exactly one group), so only fit and coverage matter.
function validateLabels() {
  let ok = true;
  for (const g of slotGroups) {
    g.error = null;
    if (g.type === "none") continue;
    const rect = rectForGroup(g);
    if (!lastPlacementSlots) {
      g.error = "Preview to check fit";
      ok = false;
      continue;
    }
    if (!rect || !rect.available) {
      g.error = "Covered by route / GLW";
      ok = false;
      continue;
    }
    if (g.type === "label") {
      const c = g.config;
      if (c.mw != null && c.mh != null) {
        if (c.mw > rect.free_width || c.mh > rect.free_height) {
          g.error = `Text ${c.mw}×${c.mh} too big for ${rect.free_width}×${rect.free_height}`;
          ok = false;
        }
      }
    } else if (g.type === "logo") {
      const c = g.config;
      const w = c.w * c.scale;
      const h = c.h * c.scale;
      if (w > rect.free_width || h > rect.free_height) {
        g.error = `Logo ${w}×${h} too big for ${rect.free_width}×${rect.free_height}`;
        ok = false;
      }
    }
  }
  setGenerateEnabled(ok);
  redrawSlotsOverlay();
}

// Rebuild label groups from saved settings (regenerate). Assumes the slot
// state was reset by applySettings first; remeasures asynchronously.
function applyLabels(labels) {
  if (Array.isArray(labels)) {
    for (const l of labels) {
      const g = ensureGroupForSlots(
        l.slots && l.slots.length ? l.slots : [l.slot],
      );
      g.type = "label";
      g.config = {
        lines: Array.isArray(l.lines) ? l.lines : [],
        font_id: l.font_id,
        font_px: l.font_px,
        color: l.color || "#ffffff",
        h_align: l.h_align || slotDefaultAlign(primaryAnchor(g)).h,
        v_align: l.v_align || slotDefaultAlign(primaryAnchor(g)).v,
        mw: null,
        mh: null,
      };
    }
  }
  remeasureAllLabels().catch(() => {});
}

// Open the text-label editor modal for a group (prefilled when editing) and,
// on save, attach the label to the group.
async function editLabel(g) {
  const anchor = primaryAnchor(g);
  const existing = g.type === "label" ? g.config : null;
  const def = slotDefaultAlign(anchor);
  const value = await formModal({
    title: existing ? "Edit text label" : "Add text label",
    okText: existing ? "Save" : "Add",
    build: (dialog) => {
      const form = $(
        "label-modal-template",
      ).content.firstElementChild.cloneNode(true);
      dialog.appendChild(form);
      const lines = form.querySelector(".lm-lines");
      const font = form.querySelector(".lm-font");
      const size = form.querySelector(".lm-size");
      const color = form.querySelector(".lm-color");
      const ha = form.querySelector(".lm-halign");
      const va = form.querySelector(".lm-valign");
      const fit = form.querySelector(".lm-fit");
      populateFontSelect(font, existing ? existing.font_id : undefined);
      if (existing) {
        lines.value = existing.lines.join("\n");
        size.value = existing.font_px;
        color.value = existing.color;
        ha.value = existing.h_align;
        va.value = existing.v_align;
      } else {
        ha.value = def.h;
        va.value = def.v;
      }
      const rect = rectForGroup(g);
      const refresh = debounce(async () => {
        const txt = lines.value.split("\n");
        const px = parseFloat(size.value);
        if (txt.every((l) => l.trim() === "") || !font.value || !(px > 0)) {
          fit.textContent = "";
          return;
        }
        try {
          const m = await measureText(font.value, px, txt);
          let s = `Text ${m.width}×${m.height}px`;
          if (rect && rect.available) {
            s += ` — slot free ${rect.free_width}×${rect.free_height}px`;
            if (m.width > rect.free_width || m.height > rect.free_height)
              s += " (too large)";
          }
          fit.textContent = s;
        } catch (err) {
          fit.textContent = `Could not measure: ${err.message}`;
        }
      }, 250);
      lines.addEventListener("input", refresh);
      size.addEventListener("input", refresh);
      font.addEventListener("change", refresh);
      refresh();
      return async () => {
        const txt = lines.value.split("\n");
        if (txt.every((l) => l.trim() === "")) {
          fit.textContent = "Enter some text.";
          return null;
        }
        const px = parseFloat(size.value);
        if (!(px > 0)) {
          fit.textContent = "Enter a positive size.";
          return null;
        }
        if (!font.value) {
          fit.textContent = "Pick a font.";
          return null;
        }
        let m;
        try {
          m = await measureText(font.value, px, txt);
        } catch (err) {
          fit.textContent = `Could not measure: ${err.message}`;
          return null;
        }
        return {
          lines: txt,
          font_id: font.value,
          font_px: px,
          color: color.value,
          h_align: ha.value,
          v_align: va.value,
          mw: m.width,
          mh: m.height,
        };
      };
    },
  });
  if (value === null) return;
  assignGroup(g, "label", value);
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

// Load the logos for the active save_to scope, shared by the logo modal.
// Logos must live in the same library as the render (same-scope rule), so this
// re-runs whenever save_to changes.
async function loadLogosForScope() {
  const scope = $("save_to") ? $("save_to").value || "personal" : "personal";
  try {
    const resp = await fetch(`/api/logos?scope=${encodeURIComponent(scope)}`);
    loadedLogos = resp.ok ? (await resp.json()).logos || [] : [];
  } catch (_err) {
    loadedLogos = [];
  }
  validateLabels();
}

// Read the logo groups into the render request shape. With
// `includeErrored = false` the groups that currently fail validation (e.g. a
// logo too large for the slot) are skipped, so one overflowing logo cannot fail
// the whole preview batch.
function readLogos(includeErrored = true) {
  const out = [];
  for (const g of slotGroups) {
    if (g.type !== "logo") continue;
    if (!includeErrored && g.error) continue;
    const c = g.config;
    out.push({
      slot: primaryAnchor(g),
      slots: g.slots.slice(),
      logo_id: c.logo_id,
      scale: c.scale,
      h_align: c.h_align,
      v_align: c.v_align,
    });
  }
  return out;
}

// Intrinsic pixel size of a loaded logo by id, or null when unknown.
function logoSizeOf(logoId) {
  const l = loadedLogos.find((x) => x.logo_id === logoId);
  return l ? { w: l.width, h: l.height } : null;
}

// Rebuild logo groups from saved settings (regenerate). Assumes the slot state
// was reset by applySettings first.
function applyLogos(logos) {
  if (Array.isArray(logos)) {
    for (const l of logos) {
      const g = ensureGroupForSlots(
        l.slots && l.slots.length ? l.slots : [l.slot],
      );
      const size = logoSizeOf(l.logo_id) || { w: 0, h: 0 };
      g.type = "logo";
      g.config = {
        logo_id: l.logo_id,
        scale: l.scale || 1,
        h_align: l.h_align || slotDefaultAlign(primaryAnchor(g)).h,
        v_align: l.v_align || slotDefaultAlign(primaryAnchor(g)).v,
        w: size.w,
        h: size.h,
      };
    }
  }
  validateLabels();
}

// Open the logo editor modal for a group (prefilled when editing) and, on
// save, attach the logo to the group.
async function editLogo(g) {
  const anchor = primaryAnchor(g);
  const existing = g.type === "logo" ? g.config : null;
  const def = slotDefaultAlign(anchor);
  const value = await formModal({
    title: existing ? "Edit logo" : "Add logo",
    okText: existing ? "Save" : "Add",
    build: (dialog) => {
      const form = $("logo-modal-template").content.firstElementChild.cloneNode(
        true,
      );
      dialog.appendChild(form);
      const pick = form.querySelector(".gm-pick");
      const scale = form.querySelector(".gm-scale");
      const ha = form.querySelector(".gm-halign");
      const va = form.querySelector(".gm-valign");
      const preview = form.querySelector(".gm-preview");
      const fit = form.querySelector(".gm-fit");
      const uploadName = form.querySelector(".gm-upload-name");
      const uploadFile = form.querySelector(".gm-upload-file");
      const uploadStatus = form.querySelector(".gm-upload-status");
      const sourceTabs = form.querySelectorAll(".gm-source-tab");
      const sourcePanels = form.querySelectorAll(".gm-source-panel");
      populateLogoSelect(pick, existing ? existing.logo_id : undefined);
      if (existing) {
        scale.value = String(existing.scale);
        ha.value = existing.h_align;
        va.value = existing.v_align;
      } else {
        ha.value = def.h;
        va.value = def.v;
      }
      const rect = rectForGroup(g);
      // Which source the two subtabs select: "reuse" a saved logo or "upload" a
      // new one (resolved when the user confirms the modal, like the notecard
      // source subtabs). Default to upload only when adding into an empty
      // library, so an empty picker is not the first thing shown.
      let source = !existing && loadedLogos.length === 0 ? "upload" : "reuse";
      // Object URL + intrinsic size of the file chosen on the upload tab, so the
      // preview and fit check work before the file is actually uploaded.
      let uploadUrl = null;
      let uploadDims = null;
      const setPreview = (src) => {
        if (src) {
          preview.src = src;
          preview.classList.remove("hidden");
        } else {
          preview.removeAttribute("src");
          preview.classList.add("hidden");
        }
      };
      const refresh = () => {
        let dims = null;
        if (source === "upload") {
          setPreview(uploadUrl);
          dims = uploadDims;
        } else {
          setPreview(pick.value ? `/api/logos/${pick.value}/image` : null);
          const opt = pick.selectedOptions[0];
          if (pick.value && opt)
            dims = {
              w: parseInt(opt.dataset.w, 10) || 0,
              h: parseInt(opt.dataset.h, 10) || 0,
            };
        }
        const s = parseInt(scale.value, 10) || 1;
        const w = (dims ? dims.w : 0) * s;
        const h = (dims ? dims.h : 0) * s;
        if (w && h) {
          let t = `Logo ${w}×${h}px`;
          if (rect && rect.available) {
            t += ` — slot free ${rect.free_width}×${rect.free_height}px`;
            if (w > rect.free_width || h > rect.free_height)
              t += " (too large)";
          }
          fit.textContent = t;
        } else {
          fit.textContent = "";
        }
      };
      const activateSource = (name) => {
        source = name;
        sourceTabs.forEach((t) =>
          t.classList.toggle("active", t.dataset.logoSource === name),
        );
        sourcePanels.forEach((p) =>
          p.classList.toggle("active", p.dataset.logoSource === name),
        );
        refresh();
      };
      sourceTabs.forEach((t) =>
        t.addEventListener("click", () => activateSource(t.dataset.logoSource)),
      );
      pick.addEventListener("change", refresh);
      scale.addEventListener("change", refresh);
      uploadFile.addEventListener("change", () => {
        if (uploadUrl) URL.revokeObjectURL(uploadUrl);
        uploadUrl = null;
        uploadDims = null;
        uploadStatus.textContent = "";
        const file = uploadFile.files && uploadFile.files[0];
        if (file) {
          uploadUrl = URL.createObjectURL(file);
          const probe = new Image();
          probe.onload = () => {
            uploadDims = { w: probe.naturalWidth, h: probe.naturalHeight };
            refresh();
          };
          probe.src = uploadUrl;
        }
        refresh();
      });
      activateSource(source);
      return async () => {
        if (source === "upload") {
          const name = uploadName.value.trim();
          const file = uploadFile.files && uploadFile.files[0];
          if (!name) {
            uploadStatus.textContent = "Enter a name for the new logo.";
            return null;
          }
          if (!file) {
            uploadStatus.textContent = "Choose an image file to upload.";
            return null;
          }
          // Logos must share the render's library (same-scope rule), so the
          // upload targets the current save_to scope.
          const scope = $("save_to")
            ? $("save_to").value || "personal"
            : "personal";
          const fd = new FormData();
          fd.append("scope", scope);
          fd.append("name", name);
          fd.append("file", file);
          uploadStatus.textContent = "Uploading…";
          let resp;
          try {
            resp = await fetch("/api/logos", { method: "POST", body: fd });
          } catch (_err) {
            uploadStatus.textContent = "Upload failed; check your connection.";
            return null;
          }
          if (!resp.ok) {
            uploadStatus.textContent = "";
            await showError(resp);
            return null;
          }
          const body = await resp.json().catch(() => ({}));
          const logo = body.logo || {};
          // Make the new logo available to the picker for later edits.
          await loadLogosForScope();
          return {
            logo_id: logo.logo_id,
            scale: parseInt(scale.value, 10) || 1,
            h_align: ha.value,
            v_align: va.value,
            w: logo.width || 0,
            h: logo.height || 0,
          };
        }
        if (!pick.value) {
          fit.textContent = "Choose a logo.";
          return null;
        }
        const opt = pick.selectedOptions[0];
        return {
          logo_id: pick.value,
          scale: parseInt(scale.value, 10) || 1,
          h_align: ha.value,
          v_align: va.value,
          w: opt ? parseInt(opt.dataset.w, 10) || 0 : 0,
          h: opt ? parseInt(opt.dataset.h, 10) || 0 : 0,
        };
      };
    },
  });
  if (value === null) return;
  assignGroup(g, "logo", value);
}

// Build a small inline-SVG icon (stroke = currentColor) for a slot button.
function svgIcon(name) {
  const NS = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(NS, "svg");
  svg.setAttribute("viewBox", "0 0 16 16");
  svg.setAttribute("width", "14");
  svg.setAttribute("height", "14");
  svg.setAttribute("aria-hidden", "true");
  const add = (tag, attrs) => {
    const el = document.createElementNS(NS, tag);
    for (const k of Object.keys(attrs)) el.setAttribute(k, String(attrs[k]));
    el.setAttribute("fill", "none");
    el.setAttribute("stroke", "currentColor");
    el.setAttribute("stroke-width", "1.5");
    el.setAttribute("stroke-linecap", "round");
    el.setAttribute("stroke-linejoin", "round");
    svg.appendChild(el);
  };
  switch (name) {
    case "none":
      add("circle", { cx: 8, cy: 8, r: 6 });
      add("line", { x1: 4, y1: 4, x2: 12, y2: 12 });
      break;
    case "label":
      add("path", { d: "M4 4h8" });
      add("path", { d: "M8 4v8" });
      break;
    case "logo":
      add("rect", { x: 2.5, y: 3, width: 11, height: 10, rx: 1 });
      add("path", { d: "M3 11l3-3 2 2 2.5-2.5L13 11" });
      add("circle", { cx: 6, cy: 6, r: 1 });
      break;
    case "legend":
      add("path", { d: "M3 4.5h10" });
      add("path", { d: "M3 8h10" });
      add("path", { d: "M3 11.5h7" });
      break;
    case "combine":
      add("path", { d: "M3 8h6" });
      add("path", { d: "M9 5l3 3-3 3" });
      break;
    case "split":
      add("path", { d: "M13 8H7" });
      add("path", { d: "M7 5L4 8l3 3" });
      break;
    case "copy":
      add("rect", { x: 5.5, y: 5.5, width: 7.5, height: 7.5, rx: 1 });
      add("path", { d: "M3 10.5V3h7.5" });
      break;
    case "swap":
      add("path", { d: "M4 6h8l-2.5-2.5" });
      add("path", { d: "M12 10H4l2.5 2.5" });
      break;
    default:
      break;
  }
  return svg;
}

// An icon-only button with a hover tooltip and an optional active state.
function iconButton(name, title, active, onClick) {
  const b = document.createElement("button");
  b.type = "button";
  b.className = "icon-btn" + (active ? " active" : "");
  b.title = title;
  b.setAttribute("aria-label", title);
  b.appendChild(svgIcon(name));
  b.addEventListener("click", (e) => {
    e.stopPropagation();
    onClick();
  });
  return b;
}

// Dispatch a slot radio-button click to the right editor / action.
function onSlotButton(g, type) {
  if (type === "none") clearGroup(g);
  else if (type === "legend") setLegend(g);
  else if (type === "label") editLabel(g);
  else if (type === "logo") editLogo(g);
}

// Re-draw the slot overlay on the current preview viewport.
function redrawSlotsOverlay() {
  const vp = $("preview-container").querySelector(".viewport");
  if (vp) drawSlotsOverlay(vp);
}

// Whether two pixel rectangles actually touch along an edge (share more than a
// single corner point), within a 1px tolerance. Rectangles separated by a gap
// — even if their slots are conceptually adjacent — do not touch.
function rectsTouch(a, b) {
  const T = 1;
  const xGap = Math.max(a.x, b.x) - Math.min(a.x + a.width, b.x + b.width);
  const yGap = Math.max(a.y, b.y) - Math.min(a.y + a.height, b.y + b.height);
  if (xGap > T || yGap > T) return false; // a real gap on either axis
  // share an edge segment (overlap on at least one axis), not just a corner
  return -xGap > T || -yGap > T;
}

// Whether the slots of two groups together fill a solid axis-aligned rectangle
// in the conceptual 3x3 grid (so combining never produces an L-shape or a
// staggered region). Groups are disjoint, so this holds iff every cell of the
// union's bounding box is occupied.
function groupsFormRectangle(ga, gb) {
  const cells = [...ga.slots, ...gb.slots].map((s) => SLOT_CELL[s]);
  const cols = cells.map((c) => c[0]);
  const rows = cells.map((c) => c[1]);
  const c0 = Math.min(...cols);
  const c1 = Math.max(...cols);
  const r0 = Math.min(...rows);
  const r1 = Math.max(...rows);
  if (cells.length !== (c1 - c0 + 1) * (r1 - r0 + 1)) return false;
  const present = new Set(cells.map(([c, r]) => `${c},${r}`));
  for (let c = c0; c <= c1; c++) {
    for (let r = r0; r <= r1; r++) {
      if (!present.has(`${c},${r}`)) return false;
    }
  }
  return true;
}

// Combine buttons on the shared edges of adjacent free slots that belong to
// different groups, but only where the two groups' free rectangles actually
// touch AND their slots together form a solid rectangle (no L-shapes or
// staggers). One button per group pair, at the midpoint of the touching slots'
// cell centres.
function drawCombineButtons(layer, bx, by, bw, bh) {
  const seen = new Set();
  for (let i = 0; i < SLOT_ANCHORS.length; i++) {
    for (let j = i + 1; j < SLOT_ANCHORS.length; j++) {
      const a = SLOT_ANCHORS[i];
      const b = SLOT_ANCHORS[j];
      const [ca, ra] = SLOT_CELL[a];
      const [cb, rb] = SLOT_CELL[b];
      if (Math.abs(ca - cb) + Math.abs(ra - rb) !== 1) continue; // not adjacent
      const ga = groupOf(a);
      const gb = groupOf(b);
      if (!ga || !gb || ga === gb) continue; // already combined
      const key = ga.id < gb.id ? `${ga.id}-${gb.id}` : `${gb.id}-${ga.id}`;
      if (seen.has(key)) continue; // one button per group pair
      const ra2 = rectForGroup(ga);
      const rb2 = rectForGroup(gb);
      if (!ra2 || !ra2.available || !ra2.free_rect) continue;
      if (!rb2 || !rb2.available || !rb2.free_rect) continue;
      // Only combinable when the rectangles really touch (no gap between them)
      // and the result is a solid rectangle.
      if (!rectsTouch(ra2.free_rect, rb2.free_rect)) continue;
      if (!groupsFormRectangle(ga, gb)) continue;
      seen.add(key);
      const cx = bx + ((ca + cb) / 2 + 0.5) * (bw / 3);
      const cy = by + ((ra + rb) / 2 + 0.5) * (bh / 3);
      const btn = iconButton(
        "combine",
        `Combine ${groupName(ga)} + ${groupName(gb)}`,
        false,
        () => combineSlots(a, b),
      );
      btn.classList.add("slot-combine");
      btn.style.left = `${cx.toFixed(1)}px`;
      btn.style.top = `${cy.toFixed(1)}px`;
      layer.appendChild(btn);
    }
  }
}

// Draw (or clear) the free-slot overlay on a preview viewport: one box per
// placement group at its (combined) free rectangle, green normally and red when
// the group's content does not fit; each box carries the None / Text / Logo /
// Legend radio buttons (and a split button when combined), plus combine buttons
// on the shared edges of adjacent free slots. Occupied single slots get a red
// "occupied" marker. Gated on the "Show free slots" toggle.
function drawSlotsOverlay(viewport) {
  if (!viewport) return;
  const existing = viewport.querySelector(".slots-overlay");
  if (existing) existing.remove();
  const toggle = $("show_slots");
  if (!toggle || !toggle.checked) return;
  if (!lastPlacementSlots || !lastPlacementImageSize) return;
  const bx = parseFloat(viewport.dataset.boundsX);
  const by = parseFloat(viewport.dataset.boundsY);
  const bw = parseFloat(viewport.dataset.boundsW);
  const bh = parseFloat(viewport.dataset.boundsH);
  const imgW = lastPlacementImageSize.width;
  const imgH = lastPlacementImageSize.height;
  if (![bx, by, bw, bh].every(Number.isFinite) || !(imgW > 0) || !(imgH > 0)) {
    return;
  }
  const sx = bw / imgW;
  const sy = bh / imgH;

  const layer = document.createElement("div");
  layer.className = "slots-overlay";

  for (const g of slotGroups) {
    const rect = rectForGroup(g);
    if (!rect || !rect.available || !rect.free_rect) {
      if (g.slots.length === 1) {
        const [col, row] = SLOT_CELL[g.slots[0]];
        const marker = document.createElement("span");
        marker.className = "slot-marker occupied";
        marker.textContent = `${SLOT_LABELS[g.slots[0]]} occupied`;
        marker.style.left = `${(bx + (col + 0.5) * (bw / 3)).toFixed(1)}px`;
        marker.style.top = `${(by + (row + 0.5) * (bh / 3)).toFixed(1)}px`;
        layer.appendChild(marker);
      }
      continue;
    }
    const fr = rect.free_rect;
    const box = document.createElement("div");
    box.className = "slot-box" + (g.error ? " error" : "");
    box.style.left = `${(bx + fr.x * sx).toFixed(1)}px`;
    box.style.top = `${(by + fr.y * sy).toFixed(1)}px`;
    box.style.width = `${(fr.width * sx).toFixed(1)}px`;
    box.style.height = `${(fr.height * sy).toFixed(1)}px`;

    const label = document.createElement("span");
    label.className = "slot-label";
    label.textContent =
      `${groupName(g)} · ${rect.free_width}×${rect.free_height}` +
      (g.error ? ` — ${g.error}` : "");
    // Full text as a tooltip so it stays readable when a narrow slot clips it.
    label.title = label.textContent;
    box.appendChild(label);

    const btns = document.createElement("div");
    btns.className = "slot-buttons";
    const radios = [
      ["none", "None", "none"],
      ["label", "Text label", "label"],
      ["logo", "Logo", "logo"],
      ["legend", "GLW legend", "legend"],
    ];
    for (const [type, title, icon] of radios) {
      const b = iconButton(icon, title, g.type === type, () =>
        onSlotButton(g, type),
      );
      b.classList.add("slot-radio");
      btns.appendChild(b);
    }
    if (g.type !== "none") {
      btns.appendChild(
        iconButton("copy", "Copy these settings to another slot", false, () =>
          startSlotAction("copy", g),
        ),
      );
    }
    btns.appendChild(
      iconButton("swap", "Swap settings with another slot", false, () =>
        startSlotAction("swap", g),
      ),
    );
    if (g.slots.length > 1) {
      const sp = iconButton("split", "Split combined slot", false, () =>
        splitGroup(g),
      );
      sp.classList.add("slot-split");
      btns.appendChild(sp);
    }
    box.appendChild(btns);
    // While a copy/swap is pending, clicking a slot box (not its buttons) picks
    // it as the target.
    if (pendingSlotAction && pendingSlotAction.from !== g) {
      box.classList.add("pick-target");
      box.addEventListener("click", () => completeSlotAction(g));
    }
    layer.appendChild(box);
  }

  drawCombineButtons(layer, bx, by, bw, bh);
  if (pendingSlotAction) layer.classList.add("picking");
  viewport.appendChild(layer);
}

// A pending copy/swap action waiting for the user to click a target slot, or
// null. { kind: "copy" | "swap", from: group }
let pendingSlotAction = null;

// Begin a copy or swap from group `g`; the next slot box click is the target.
function startSlotAction(kind, g) {
  pendingSlotAction = { kind, from: g };
  const st = $("placement-status");
  if (st)
    st.textContent =
      kind === "copy"
        ? "Click another slot to copy these settings into (Esc to cancel)."
        : "Click another slot to swap settings with (Esc to cancel).";
  redrawSlotsOverlay();
}

// Cancel any pending copy/swap.
function cancelSlotAction() {
  if (!pendingSlotAction) return;
  pendingSlotAction = null;
  const st = $("placement-status");
  if (st) st.textContent = "";
  redrawSlotsOverlay();
}

// Apply the pending copy/swap onto the target group.
function completeSlotAction(target) {
  if (!pendingSlotAction) return;
  const { kind, from } = pendingSlotAction;
  pendingSlotAction = null;
  const st = $("placement-status");
  if (st) st.textContent = "";
  if (target === from) {
    redrawSlotsOverlay();
    return;
  }
  if (kind === "copy") {
    const config = from.config ? JSON.parse(JSON.stringify(from.config)) : null;
    assignGroup(target, from.type, config);
  } else {
    const tType = target.type;
    const tConfig = target.config;
    target.type = from.type;
    target.config = from.config;
    from.type = tType;
    from.config = tConfig;
    refreshPlacement();
  }
}

// Combine the groups containing slots `a` and `b` into one, keeping the
// configured element (asking which when both are set), then refresh the slots.
async function combineSlots(a, b) {
  const ga = groupOf(a);
  const gb = groupOf(b);
  if (!ga || !gb || ga === gb) return;
  let type = "none";
  let config = null;
  const aHas = ga.type !== "none";
  const bHas = gb.type !== "none";
  if (aHas && bHas) {
    const keep = await choiceModal({
      title: "Combine slots",
      message: "Both slots have content. Which should the combined slot keep?",
      choices: [
        { label: `Keep ${describe(ga)}`, value: "a" },
        { label: `Keep ${describe(gb)}`, value: "b" },
      ],
    });
    if (keep === null) return;
    [type, config] = keep === "a" ? [ga.type, ga.config] : [gb.type, gb.config];
  } else if (aHas) {
    [type, config] = [ga.type, ga.config];
  } else if (bHas) {
    [type, config] = [gb.type, gb.config];
  }
  const slots = sortAnchors([...ga.slots, ...gb.slots]);
  removeGroupObj(ga);
  removeGroupObj(gb);
  addGroup(slots, type, config);
  await findFreeSlots();
  refreshPlacementPreview();
}

// Split a combined group back into singleton slots, keeping its element on the
// primary anchor (validation flags it red if it no longer fits there).
async function splitGroup(g) {
  const { type, config } = g;
  const primary = primaryAnchor(g);
  const slots = g.slots.slice();
  removeGroupObj(g);
  let primaryGroup = null;
  for (const slot of slots) {
    const ng = addGroup([slot], "none", null);
    if (slot === primary) primaryGroup = ng;
  }
  if (type !== "none" && primaryGroup) {
    primaryGroup.type = type;
    primaryGroup.config = config;
  }
  await findFreeSlots();
  refreshPlacementPreview();
}

// Compute free placement slots for the active tab (grid or notecard),
// mirroring the corresponding render request, then redraw the preview overlay
// and re-validate the labels.
async function findFreeSlots() {
  const statusEl = $("placement-status");
  statusEl.textContent = "Computing free slots…";
  try {
    const activeTab = document.querySelector(".tab.active");
    const which = activeTab ? activeTab.dataset.tab : "grid";
    // The combined slot groups whose rectangles we need reported back.
    const groupSlots = slotGroups
      .filter((g) => g.slots.length > 1)
      .map((g) => g.slots);
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
      if (groupSlots.length) body.groups = groupSlots;
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
      if (groupSlots.length)
        fd.append("groups_json", JSON.stringify(groupSlots));
      resp = await fetch("/api/render/placement-slots/usb-notecard", {
        method: "POST",
        body: fd,
      });
    }
    if (!resp.ok) throw new Error(await resp.text());
    const data = await resp.json();
    lastPlacementSlots = {};
    for (const s of data.slots) lastPlacementSlots[s.slot] = s;
    lastGroupRects = {};
    for (const gr of data.groups || []) lastGroupRects[groupKey(gr.slots)] = gr;
    lastPlacementImageSize = {
      width: data.image_width,
      height: data.image_height,
    };
    statusEl.textContent = `Free slots for a ${data.image_width}×${data.image_height}px render.`;
    // Validate first (sets each group's fit error), then draw the labels/logos
    // overlay so it includes only the placements that currently fit.
    validateLabels();
    const vp = $("preview-container").querySelector(".viewport");
    if (vp && lastPreviewRect) drawPlacementOverlay(vp, lastPreviewRect);
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
  initSlotGroups();
  loadLogosForScope().catch(() => {});
  // Escape cancels a pending copy/swap (only when no modal is open — the modal
  // handles its own Escape).
  document.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && pendingSlotAction) cancelSlotAction();
  });
  const showSlots = $("show_slots");
  if (showSlots)
    showSlots.addEventListener("change", () => redrawSlotsOverlay());
  // GLW on/off changes whether the legend draws and changes slot occupancy, so
  // re-validate and refresh the preview overlays.
  const glwEnabled = $("glw_enabled");
  if (glwEnabled)
    glwEnabled.addEventListener("change", () => {
      validateLabels();
      refreshPlacementPreview();
    });
  // The slot occupancy differs between the grid and notecard tabs, so a tab
  // switch invalidates the cached result (the placements themselves persist).
  document.querySelectorAll(".tab").forEach((tab) => {
    tab.addEventListener("click", () => {
      lastPlacementSlots = null;
      lastGroupRects = {};
      lastPlacementImageSize = null;
      const st = $("placement-status");
      if (st) st.textContent = "";
      validateLabels();
    });
  });
  validateLabels();
});
