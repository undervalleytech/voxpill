import "./contextMenuGuard.js";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

const pill = document.getElementById("profile-pill");
const textEl = document.getElementById("profile-pill-text");
const win = getCurrentWebviewWindow();

const prefersReducedMotion =
  typeof window.matchMedia === "function" &&
  window.matchMedia("(prefers-reduced-motion: reduce)").matches;

let hideTimer = null;

function scheduleHide() {
  if (hideTimer !== null) {
    clearTimeout(hideTimer);
  }
  hideTimer = window.setTimeout(() => {
    hideTimer = null;
    void win.hide().catch(() => {});
  }, 2500);
}

function applyProfileLabel(name, animForward, reformulationEnabled) {
  if (!textEl) return;
  textEl.classList.remove("enter-from-right", "enter-from-left", "enter-fade");
  textEl.textContent = name;
  if (reformulationEnabled) {
    pill?.setAttribute("aria-label", `Active profile: ${name}`);
  } else {
    pill?.setAttribute("aria-label", "AI reformulation disabled");
  }

  requestAnimationFrame(() => {
    requestAnimationFrame(() => {
      if (prefersReducedMotion) {
        textEl.classList.add("enter-fade");
      } else {
        textEl.classList.add(animForward ? "enter-from-right" : "enter-from-left");
      }
    });
  });
  scheduleHide();
}

listen("profile-pill-update", (ev) => {
  const p = ev.payload;
  const name = p && typeof p.profile_name === "string" ? p.profile_name : "";
  const animForward = Boolean(p && p.anim_forward);
  const reformulationEnabled =
    p && typeof p.reformulation_enabled === "boolean" ? p.reformulation_enabled : true;
  applyProfileLabel(name || "—", animForward, reformulationEnabled);
}).catch(() => {});
