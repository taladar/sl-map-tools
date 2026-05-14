"use strict";

// In-page replacements for window.confirm / prompt / alert. Every string
// flows in via textContent / .value, so an attacker-controlled name in a
// dialog body cannot inject markup or layout-breaking newlines.
//
// Each helper returns a Promise:
//   confirmModal(...) -> boolean
//   promptModal(...)  -> string | null   (null on cancel)
//   alertModal(...)   -> undefined       (resolves on dismiss)

let overlayEl = null;
let activeOnCancel = null;
let previouslyFocused = null;

function ensureOverlay() {
  if (overlayEl) return overlayEl;
  overlayEl = document.createElement("div");
  overlayEl.className = "modal-overlay hidden";
  overlayEl.setAttribute("role", "dialog");
  overlayEl.setAttribute("aria-modal", "true");
  overlayEl.addEventListener("click", (e) => {
    if (e.target === overlayEl) closeModal(true);
  });
  document.addEventListener("keydown", (e) => {
    if (!overlayEl || overlayEl.classList.contains("hidden")) return;
    if (e.key === "Escape") {
      e.preventDefault();
      closeModal(true);
    }
  });
  document.body.appendChild(overlayEl);
  return overlayEl;
}

function closeModal(cancelled) {
  if (!overlayEl) return;
  overlayEl.classList.add("hidden");
  overlayEl.replaceChildren();
  if (previouslyFocused && typeof previouslyFocused.focus === "function") {
    try {
      previouslyFocused.focus();
    } catch (_e) {
      // ignore focus restoration errors
    }
  }
  previouslyFocused = null;
  const onCancel = activeOnCancel;
  activeOnCancel = null;
  if (cancelled && onCancel) onCancel();
}

function openModal(buildBody) {
  const overlay = ensureOverlay();
  overlay.replaceChildren();
  previouslyFocused = document.activeElement;
  const dialog = document.createElement("div");
  dialog.className = "modal-dialog";
  dialog.setAttribute("role", "document");
  buildBody(dialog);
  overlay.appendChild(dialog);
  overlay.classList.remove("hidden");
}

function setHeader(dialog, opts) {
  if (opts.title) {
    const h = document.createElement("h2");
    h.className = "modal-title";
    h.textContent = opts.title;
    dialog.appendChild(h);
  }
  if (opts.message != null) {
    const p = document.createElement("p");
    p.className = "modal-message";
    p.textContent = opts.message;
    dialog.appendChild(p);
  }
}

function makeFooter(buttons) {
  const footer = document.createElement("div");
  footer.className = "modal-footer";
  for (const b of buttons) {
    const btn = document.createElement("button");
    btn.type = "button";
    btn.className = b.className || "modal-btn";
    btn.textContent = b.text;
    btn.addEventListener("click", b.onClick);
    footer.appendChild(btn);
  }
  return footer;
}

function confirmModal(opts) {
  return new Promise((resolve) => {
    activeOnCancel = () => resolve(false);
    openModal((dialog) => {
      setHeader(dialog, opts);
      const finish = (value) => {
        activeOnCancel = null;
        closeModal(false);
        resolve(value);
      };
      const okClass = opts.danger
        ? "modal-btn primary danger"
        : "modal-btn primary";
      const footer = makeFooter([
        {
          text: "Cancel",
          className: "modal-btn",
          onClick: () => finish(false),
        },
        {
          text: opts.okText || "OK",
          className: okClass,
          onClick: () => finish(true),
        },
      ]);
      dialog.appendChild(footer);
      setTimeout(() => {
        const okBtn = footer.lastChild;
        if (okBtn && typeof okBtn.focus === "function") okBtn.focus();
      }, 0);
    });
  });
}

function promptModal(opts) {
  return new Promise((resolve) => {
    activeOnCancel = () => resolve(null);
    openModal((dialog) => {
      setHeader(dialog, opts);
      const input = document.createElement("input");
      input.type = "text";
      input.className = "modal-input";
      input.value = opts.default || "";
      const finish = (value) => {
        activeOnCancel = null;
        closeModal(false);
        resolve(value);
      };
      input.addEventListener("keydown", (e) => {
        if (e.key === "Enter") {
          e.preventDefault();
          finish(input.value);
        }
      });
      dialog.appendChild(input);
      const footer = makeFooter([
        {
          text: "Cancel",
          className: "modal-btn",
          onClick: () => finish(null),
        },
        {
          text: opts.okText || "OK",
          className: "modal-btn primary",
          onClick: () => finish(input.value),
        },
      ]);
      dialog.appendChild(footer);
      setTimeout(() => input.focus(), 0);
    });
  });
}

function alertModal(opts) {
  return new Promise((resolve) => {
    activeOnCancel = () => resolve();
    openModal((dialog) => {
      setHeader(dialog, opts);
      const finish = () => {
        activeOnCancel = null;
        closeModal(false);
        resolve();
      };
      const footer = makeFooter([
        {
          text: opts.okText || "OK",
          className: "modal-btn primary",
          onClick: finish,
        },
      ]);
      dialog.appendChild(footer);
      setTimeout(() => {
        const okBtn = footer.lastChild;
        if (okBtn && typeof okBtn.focus === "function") okBtn.focus();
      }, 0);
    });
  });
}

// Like alertModal, but the caller fills the dialog body itself via the
// `build(dialog)` callback (which runs after the title is set and before
// the footer). Useful when the body is more than a single text line —
// tables, instructions, copy-to-clipboard fields, etc. `footerExtras`,
// if supplied, is a list of DOM nodes prepended to the footer ahead of
// the Close button — handy for putting a Download link alongside Close.
function infoModal(opts) {
  return new Promise((resolve) => {
    activeOnCancel = () => resolve();
    openModal((dialog) => {
      setHeader(dialog, { title: opts.title });
      if (typeof opts.build === "function") opts.build(dialog);
      const finish = () => {
        activeOnCancel = null;
        closeModal(false);
        resolve();
      };
      const footer = makeFooter([
        {
          text: opts.okText || "Close",
          className: "modal-btn primary",
          onClick: finish,
        },
      ]);
      if (Array.isArray(opts.footerExtras)) {
        for (const node of opts.footerExtras.slice().reverse()) {
          footer.insertBefore(node, footer.firstChild);
        }
      }
      dialog.appendChild(footer);
      setTimeout(() => {
        const okBtn = footer.lastChild;
        if (okBtn && typeof okBtn.focus === "function") okBtn.focus();
      }, 0);
    });
  });
}

// Helper for the common `alert(await resp.text())` pattern. The server's
// JSON error envelope is `{"error": "..."}` (see error.rs); we surface
// just the message field when present and fall back to the raw body so
// pre-JSON responses still render. A `Retry-After` header (sent on
// rate-limit responses) is appended as "try again in N s / N min".
async function showError(resp, fallbackTitle) {
  const raw = await resp.text();
  let msg = raw;
  try {
    const body = JSON.parse(raw);
    if (body && typeof body.error === "string") msg = body.error;
  } catch (_e) {
    // not JSON — keep the raw text
  }
  const retryHeader = resp.headers.get("retry-after");
  if (retryHeader) {
    const secs = parseInt(retryHeader, 10);
    if (Number.isFinite(secs) && secs > 0) {
      const human = secs < 90 ? `${secs} s` : `${Math.ceil(secs / 60)} min`;
      msg = `${msg} — try again in ${human}.`;
    }
  }
  await alertModal({ title: fallbackTitle || "Error", message: msg });
}
