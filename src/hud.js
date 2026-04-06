import "./contextMenuGuard.js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

const pill = document.getElementById("hud-pill");
const wave = document.getElementById("wave");
const BAR_COUNT = 28;
const bars = [];
/** Orange marque (#de3119) — waveform : dégradé vers le blanc, avec parcimonie */
const OR = { r: 222, g: 49, b: 25 };

for (let i = 0; i < BAR_COUNT; i++) {
  const b = document.createElement("div");
  b.className = "bar";
  b.style.height = "4px";
  wave.appendChild(b);
  bars.push(b);
}

/** Niveau 0–1000 depuis Rust (micro) */
let target = 0;
let smoothed = 0;
/** recording | transcribing | reformulating | idle */
let hudPhase = "idle";
/** normal | ai_assist — défini par le backend (raccourci assistant) */
let hudMode = "normal";
let lastPhaseForSound = "idle";
let statusSoundsEnabled = true;

function lerp(a, b, t) {
  return a + (b - a) * t;
}

function soundUrl(filename) {
  return new URL(`sounds/${filename}`, window.location.href).href;
}

function playSoundFile(filename) {
  const a = new Audio(soundUrl(filename));
  a.volume = 0.42;
  a.play().catch(() => {});
}

function maybePlayPhaseSound() {
  if (!statusSoundsEnabled) {
    lastPhaseForSound = hudPhase;
    return;
  }
  if (hudPhase === "recording" && lastPhaseForSound !== "recording") {
    playSoundFile("recording-start.mp3");
  } else if (
    (hudPhase === "transcribing" || hudPhase === "reformulating") &&
    lastPhaseForSound !== "transcribing" &&
    lastPhaseForSound !== "reformulating"
  ) {
    playSoundFile("transcribe-start.mp3");
  }
  lastPhaseForSound = hudPhase;
}

function updateRecordingDot() {
  if (hudPhase === "recording" && hudMode !== "ai_assist") {
    pill?.classList.add("hud-pill--recording");
  } else {
    pill?.classList.remove("hud-pill--recording");
  }
}

function updateAiAssistHalo() {
  const on =
    hudMode === "ai_assist" &&
    hudPhase !== "idle" &&
    (hudPhase === "recording" ||
      hudPhase === "transcribing" ||
      hudPhase === "reformulating");
  pill?.classList.toggle("hud-pill--ai-assist", Boolean(on));
}

function updateWaveGlow() {
  const on =
    hudPhase === "recording" || hudPhase === "transcribing" || hudPhase === "reformulating";
  pill?.classList.toggle("hud-pill--wave-glow", Boolean(on));
}

/** Rectangle de la capsule seule (pas le spacer) : clic à travers hors zone sous Windows / WebView2. */
function reportHudInteractiveRect() {
  if (!pill) return;
  const r = pill.getBoundingClientRect();
  if (r.width < 2 || r.height < 2) {
    void invoke("rec_hud_set_interactive_rect", { x: 0, y: 0, width: 0, height: 0 }).catch(() => {});
    return;
  }
  void invoke("rec_hud_set_interactive_rect", {
    x: r.left,
    y: r.top,
    width: r.width,
    height: r.height,
  }).catch(() => {});
}

function scheduleReportHudInteractiveRect() {
  requestAnimationFrame(() => reportHudInteractiveRect());
}

function setHudPhase(phase) {
  hudPhase = phase;
  const transcribingLike = phase === "transcribing" || phase === "reformulating";
  pill?.setAttribute(
    "aria-label",
    transcribingLike ? "Transcription en cours" : "Enregistrement en cours",
  );
  updateRecordingDot();
  updateAiAssistHalo();
  updateWaveGlow();
  scheduleReportHudInteractiveRect();
  maybePlayPhaseSound();
}

listen("hud-phase", (ev) => {
  const payload = ev.payload && typeof ev.payload === "object" ? ev.payload : {};
  const p = typeof payload.phase === "string" ? payload.phase : "idle";
  hudMode = payload.mode === "ai_assist" ? "ai_assist" : "normal";
  if (p === "idle") {
    hudPhase = "idle";
    lastPhaseForSound = "idle";
    hudMode = "normal";
    pill?.setAttribute("aria-label", "Dictée");
    pill?.classList.remove("hud-pill--recording", "hud-pill--ai-assist", "hud-pill--wave-glow");
    void invoke("rec_hud_set_interactive_rect", { x: 0, y: 0, width: 0, height: 0 }).catch(() => {});
    return;
  }
  setHudPhase(p);
}).catch(() => {});

listen("mic-level", (ev) => {
  const v = ev.payload && typeof ev.payload.v === "number" ? ev.payload.v : 0;
  target = Math.min(1000, Math.max(0, v));
}).catch(() => {});

listen("hud-sound-enabled", (ev) => {
  const enabled = ev.payload && typeof ev.payload.enabled === "boolean" ? ev.payload.enabled : true;
  statusSoundsEnabled = enabled;
}).catch(() => {});

const prefersReducedMotion =
  typeof window.matchMedia === "function" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

function tick() {
  const t = Date.now();
  const center = (BAR_COUNT - 1) / 2;

  if (hudPhase === "transcribing" || hudPhase === "reformulating") {
    const speed = prefersReducedMotion ? 0.08 : 0.22;
    smoothed += (500 - smoothed) * speed;
    const n = smoothed / 1000;
    bars.forEach((el, i) => {
      const dist = Math.abs(i - center) / Math.max(center, 1);
      const phase = t / 95 + i * 0.55;
      const phase2 = t / 140 + i * 0.31;
      const wobble = Math.sin(phase) * 0.42 + Math.sin(phase2) * 0.28 + 0.7;
      const h = 4 + n * (26 * wobble * (1 - dist * 0.55) + 12 * (1 - dist));
      el.style.height = `${Math.max(3, h)}px`;
      const whiteA = lerp(0.38, 0.08, dist) * 0.92;
      const foot = (0.06 + n * 0.1) * (1 - dist * 0.5);
      el.style.background = `linear-gradient(to top, rgba(${OR.r},${OR.g},${OR.b},${foot}) 0%, rgba(255,245,240,${whiteA * 0.35}) 40%, rgba(255,255,255,${whiteA}) 100%)`;
      const glow = (0.05 + n * 0.08) * (1 - dist * 0.45);
      el.style.boxShadow = `0 0 ${2 + n * 5}px rgba(${OR.r},${OR.g},${OR.b},${glow})`;
    });
  } else {
    smoothed += (target - smoothed) * 0.38;
    const n = smoothed / 1000;
    const active = n > 0.025;
    bars.forEach((el, i) => {
      const dist = Math.abs(i - center) / Math.max(center, 1);
      const phase = Math.sin(t / 120 + i * 0.35) * 0.25 + 0.75;
      const h = 4 + n * (26 * phase * (1 - dist * 0.6) + 8 * (1 - dist));
      el.style.height = `${Math.max(3, h)}px`;
      if (active) {
        const peak = Math.min(1, n * 1.35);
        const orangeFoot = (0.32 + peak * 0.38) * (1 - dist * 0.4);
        const midA = (0.18 + peak * 0.22) * (1 - dist * 0.25);
        const topA = (0.72 + peak * 0.22) * (1 - dist * 0.35);
        el.style.background = `linear-gradient(to top, rgba(${OR.r},${OR.g},${OR.b},${orangeFoot}) 0%, rgba(255,210,198,${midA}) 38%, rgba(255,255,255,${topA}) 100%)`;
        const glow = (0.06 + peak * 0.14) * (1 - dist * 0.5);
        el.style.boxShadow = `0 0 ${2 + peak * 7}px rgba(${OR.r},${OR.g},${OR.b},${glow})`;
      } else {
        el.style.boxShadow = "none";
        const a = lerp(0.08, 0.14, 1 - dist);
        el.style.background = `rgba(255,255,255,${a})`;
      }
    });
  }

  requestAnimationFrame(tick);
}

requestAnimationFrame(tick);

if (typeof ResizeObserver === "function" && pill) {
  const ro = new ResizeObserver(() => scheduleReportHudInteractiveRect());
  ro.observe(pill);
}
requestAnimationFrame(() => scheduleReportHudInteractiveRect());
window.addEventListener("focus", () => scheduleReportHudInteractiveRect());
