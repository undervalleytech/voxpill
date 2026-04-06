import "./contextMenuGuard.js";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebview } from "@tauri-apps/api/webview";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open } from "@tauri-apps/plugin-dialog";

const STORAGE_LANG = "voxpill-file-pill-lang";
const STORAGE_TRANSLATE = "voxpill-file-pill-translate-target";

const filePill = document.getElementById("file-pill");
const langSelect = document.getElementById("lang-out-select");
const langTrigger = document.getElementById("lang-out-trigger");
const langValue = document.getElementById("lang-out-value");
const langList = document.getElementById("lang-out-list");
const translateSelect = document.getElementById("translate-out-select");
const translateTrigger = document.getElementById("translate-out-trigger");
const translateValue = document.getElementById("translate-out-value");
const translateList = document.getElementById("translate-out-list");
const fileToast = document.getElementById("file-toast");
const fileStatus = document.getElementById("file-status");

/** Union des boîtes (viewport) pour la hit-zone native Windows (clic à travers hors zone). */
function unionBounds(elements) {
  let minX = Infinity;
  let minY = Infinity;
  let maxX = -Infinity;
  let maxY = -Infinity;
  for (const el of elements) {
    if (!el || !(el instanceof Element)) continue;
    if (el.hidden) continue;
    const r = el.getBoundingClientRect();
    if (r.width < 1 && r.height < 1) continue;
    minX = Math.min(minX, r.left);
    minY = Math.min(minY, r.top);
    maxX = Math.max(maxX, r.right);
    maxY = Math.max(maxY, r.bottom);
  }
  if (minX === Infinity) return null;
  return { x: minX, y: minY, width: maxX - minX, height: maxY - minY };
}

function reportInteractiveRect() {
  if (!filePill) return;
  const parts = [filePill];
  if (langList && !langList.hidden) parts.push(langList);
  if (translateList && !translateList.hidden) parts.push(translateList);
  const b = unionBounds(parts);
  if (!b || b.width < 2 || b.height < 2) {
    void invoke("file_pill_set_interactive_rect", { x: 0, y: 0, width: 0, height: 0 }).catch(() => {});
    return;
  }
  void invoke("file_pill_set_interactive_rect", {
    x: b.x,
    y: b.y,
    width: b.width,
    height: b.height,
  }).catch(() => {});
}

function scheduleReportInteractiveRect() {
  requestAnimationFrame(() => reportInteractiveRect());
}

const FILE_LANG_OPTIONS = [
  { value: "auto", label: "Auto" },
  { value: "en", label: "English" },
  { value: "fr", label: "French" },
  { value: "de", label: "German" },
  { value: "es", label: "Spanish" },
  { value: "it", label: "Italian" },
  { value: "pt", label: "Portuguese" },
  { value: "nl", label: "Dutch" },
  { value: "pl", label: "Polish" },
  { value: "ru", label: "Russian" },
  { value: "ja", label: "Japanese" },
  { value: "zh", label: "Chinese" },
  { value: "ko", label: "Korean" },
];

/** Same ISO codes as locales TRANSLATE_TARGET_OPTIONS; English labels for the file pill. */
const FILE_TRANSLATE_TARGET_OPTIONS = [
  { value: "en", label: "English" },
  { value: "fr", label: "French" },
  { value: "de", label: "German" },
  { value: "es", label: "Spanish" },
  { value: "it", label: "Italian" },
  { value: "pt", label: "Portuguese" },
  { value: "nl", label: "Dutch" },
  { value: "pl", label: "Polish" },
  { value: "ru", label: "Russian" },
  { value: "uk", label: "Ukrainian" },
  { value: "ja", label: "Japanese" },
  { value: "zh", label: "Chinese" },
  { value: "ko", label: "Korean" },
  { value: "ar", label: "Arabic" },
  { value: "hi", label: "Hindi" },
  { value: "tr", label: "Turkish" },
  { value: "sv", label: "Swedish" },
  { value: "da", label: "Danish" },
  { value: "no", label: "Norwegian" },
  { value: "fi", label: "Finnish" },
  { value: "el", label: "Greek" },
  { value: "he", label: "Hebrew" },
  { value: "cs", label: "Czech" },
  { value: "ro", label: "Romanian" },
  { value: "hu", label: "Hungarian" },
  { value: "th", label: "Thai" },
  { value: "vi", label: "Vietnamese" },
  { value: "id", label: "Indonesian" },
  { value: "ca", label: "Catalan" },
  { value: "bg", label: "Bulgarian" },
  { value: "fa", label: "Persian" },
];

const TRANSLATE_FILE_OPTIONS = [{ value: "", label: "No translation" }, ...FILE_TRANSLATE_TARGET_OPTIONS];

function loadStoredLang() {
  try {
    const raw = sessionStorage.getItem(STORAGE_LANG);
    if (raw && FILE_LANG_OPTIONS.some((o) => o.value === raw)) return raw;
  } catch {
    /* ignore */
  }
  return "auto";
}

function loadStoredTranslate() {
  try {
    const raw = sessionStorage.getItem(STORAGE_TRANSLATE);
    if (raw === "" || raw === null) return "";
    if (TRANSLATE_FILE_OPTIONS.some((o) => o.value === raw)) return raw;
  } catch {
    /* ignore */
  }
  return "";
}

let langOutValue = loadStoredLang();
let langOpen = false;
let langHighlight = 0;
let translateOutValue = loadStoredTranslate();
let translateOpen = false;
let translateHighlight = 0;
let filePhase = "idle";
let toastTimer = null;

function renderLangOptions() {
  langList.innerHTML = FILE_LANG_OPTIONS.map(
    (o, i) =>
      `<li role="option" data-value="${o.value}" data-index="${i}" aria-selected="${
        o.value === langOutValue ? "true" : "false"
      }" class="${i === langHighlight ? "is-highlighted" : ""}">${o.label}</li>`,
  ).join("");
  const selected =
    FILE_LANG_OPTIONS.find((o) => o.value === langOutValue) || FILE_LANG_OPTIONS[0];
  langValue.textContent = selected.label;
  langSelect.dataset.value = langOutValue;
}

function renderTranslateOptions() {
  translateList.innerHTML = TRANSLATE_FILE_OPTIONS.map(
    (o, i) =>
      `<li role="option" data-value="${o.value}" data-index="${i}" aria-selected="${
        o.value === translateOutValue ? "true" : "false"
      }" class="${i === translateHighlight ? "is-highlighted" : ""}">${o.label}</li>`,
  ).join("");
  const selected =
    TRANSLATE_FILE_OPTIONS.find((o) => o.value === translateOutValue) ||
    TRANSLATE_FILE_OPTIONS[0];
  translateValue.textContent = selected.label;
  translateSelect.dataset.value = translateOutValue;
}

function setLangOpen(v) {
  langOpen = v;
  langTrigger.setAttribute("aria-expanded", v ? "true" : "false");
  langList.hidden = !v;
  scheduleReportInteractiveRect();
}

function setTranslateOpen(v) {
  translateOpen = v;
  translateTrigger.setAttribute("aria-expanded", v ? "true" : "false");
  translateList.hidden = !v;
  scheduleReportInteractiveRect();
}

renderLangOptions();
renderTranslateOptions();
setLangOpen(false);
setTranslateOpen(false);

if (typeof ResizeObserver === "function" && filePill) {
  const ro = new ResizeObserver(() => scheduleReportInteractiveRect());
  ro.observe(filePill);
}
requestAnimationFrame(() => {
  requestAnimationFrame(() => scheduleReportInteractiveRect());
});
window.addEventListener("focus", () => scheduleReportInteractiveRect());

let busy = false;

function setBusy(v) {
  busy = v;
  filePill.classList.toggle("is-busy", v);
  filePill.setAttribute("aria-busy", String(v));
  langSelect.classList.toggle("is-disabled", v);
  translateSelect.classList.toggle("is-disabled", v);
  langTrigger.disabled = v;
  translateTrigger.disabled = v;
  if (v) {
    setLangOpen(false);
    setTranslateOpen(false);
  }
}

function setStatus(t) {
  fileStatus.textContent = t || "";
}

function soundUrl(filename) {
  return new URL(`sounds/${filename}`, window.location.href).href;
}

function playSoundFile(filename) {
  const a = new Audio(soundUrl(filename));
  a.volume = 0.42;
  a.play().catch(() => {});
}

function showToast(message) {
  if (!fileToast) return;
  if (toastTimer) {
    clearTimeout(toastTimer);
    toastTimer = null;
  }
  fileToast.textContent = message;
  fileToast.classList.remove("show");
  requestAnimationFrame(() => fileToast.classList.add("show"));
  toastTimer = setTimeout(() => {
    fileToast.classList.remove("show");
    toastTimer = null;
  }, 1600);
}

function setFilePhase(next) {
  const prev = filePhase;
  filePhase = next;
  filePill.classList.toggle("is-transcribing", next === "decode" || next === "transcribe");
  if (next === "decode" && prev !== "decode") {
    playSoundFile("recording-start.mp3");
  }
  if (next === "done" && prev !== "done") {
    playSoundFile("transcribe-start.mp3");
  }
}

async function runTranscribe(path) {
  if (!path || busy) return;
  setBusy(true);
  setStatus("");
  setFilePhase("decode");
  const translateEnabled = translateOutValue !== "";
  const translateTarget = translateEnabled ? translateOutValue : "";
  let invokeCancelled = false;
  try {
    const out = await invoke("transcribe_audio_file", {
      path,
      language: langOutValue || "auto",
      translateEnabled,
      translateTarget,
    });
    void out;
    setStatus("");
  } catch (e) {
    const msg = e instanceof Error ? e.message : String(e);
    if (/annulée/i.test(msg) || /cancel/i.test(msg)) {
      invokeCancelled = true;
      setFilePhase("idle");
      setStatus("");
    } else {
      setFilePhase("error");
      setStatus(msg);
    }
  } finally {
    filePill.classList.remove("is-transcribing");
    if (!invokeCancelled && filePhase !== "error") {
      setFilePhase("idle");
    }
    setBusy(false);
    scheduleReportInteractiveRect();
  }
}

async function pickAndTranscribe() {
  if (busy) return;
  const picked = await open({
    multiple: false,
    filters: [
      {
        name: "Audio",
        extensions: [
          "mp3",
          "wav",
          "flac",
          "m4a",
          "aac",
          "ogg",
          "opus",
          "wma",
          "aiff",
          "caf",
          "mp4",
          "webm",
        ],
      },
    ],
  });
  const path = typeof picked === "string" ? picked : Array.isArray(picked) ? picked[0] : null;
  if (path) {
    await runTranscribe(path);
  }
}

filePill.addEventListener("click", () => {
  void pickAndTranscribe();
});

filePill.addEventListener("keydown", (e) => {
  if (e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    void pickAndTranscribe();
  }
});

langSelect.addEventListener("click", (e) => e.stopPropagation());
langTrigger.addEventListener("click", (e) => {
  e.stopPropagation();
  if (busy) return;
  setTranslateOpen(false);
  setLangOpen(!langOpen);
});
langTrigger.addEventListener("keydown", (e) => {
  e.stopPropagation();
  if (busy) return;
  if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    setTranslateOpen(false);
    setLangOpen(true);
  } else if (e.key === "Escape") {
    e.preventDefault();
    setLangOpen(false);
  }
});

langList.addEventListener("click", (e) => {
  e.stopPropagation();
  const target = e.target.closest("li[data-value]");
  if (!target) return;
  langOutValue = target.dataset.value || "auto";
  langHighlight = Number(target.dataset.index || 0);
  renderLangOptions();
  setLangOpen(false);
  try {
    sessionStorage.setItem(STORAGE_LANG, langOutValue);
  } catch {
    /* ignore */
  }
});

translateSelect.addEventListener("click", (e) => e.stopPropagation());
translateTrigger.addEventListener("click", (e) => {
  e.stopPropagation();
  if (busy) return;
  setLangOpen(false);
  setTranslateOpen(!translateOpen);
});
translateTrigger.addEventListener("keydown", (e) => {
  e.stopPropagation();
  if (busy) return;
  if (e.key === "ArrowDown" || e.key === "Enter" || e.key === " ") {
    e.preventDefault();
    setLangOpen(false);
    setTranslateOpen(true);
  } else if (e.key === "Escape") {
    e.preventDefault();
    setTranslateOpen(false);
  }
});

translateList.addEventListener("click", (e) => {
  e.stopPropagation();
  const target = e.target.closest("li[data-value]");
  if (!target) return;
  translateOutValue = target.dataset.value ?? "";
  translateHighlight = Number(target.dataset.index || 0);
  renderTranslateOptions();
  setTranslateOpen(false);
  try {
    sessionStorage.setItem(STORAGE_TRANSLATE, translateOutValue);
  } catch {
    /* ignore */
  }
});

document.addEventListener("click", () => {
  if (langOpen) setLangOpen(false);
  if (translateOpen) setTranslateOpen(false);
});

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    if (langOpen) {
      setLangOpen(false);
      return;
    }
    if (translateOpen) {
      setTranslateOpen(false);
      return;
    }
    e.preventDefault();
    void getCurrentWebviewWindow().hide();
  }
});

void getCurrentWebview()
  .onDragDropEvent((ev) => {
    const p = ev.payload;
    if (p.type === "enter" || p.type === "over") {
      filePill.classList.add("is-drag");
    } else if (p.type === "leave") {
      filePill.classList.remove("is-drag");
    } else if (p.type === "drop") {
      filePill.classList.remove("is-drag");
      if (busy) return;
      const paths = p.paths;
      if (paths && paths[0]) {
        void runTranscribe(paths[0]);
      }
    }
  })
  .catch((err) => {
    console.error("[file-pill] onDragDropEvent", err);
  });

listen("file-transcribe-status", (ev) => {
  const p = ev.payload;
  if (!p || typeof p !== "object") return;
  const msg = typeof p.message === "string" ? p.message : "";
  if (p.status === "decode" || p.status === "transcribe") {
    setFilePhase(p.status);
    void msg;
    setStatus("");
  } else if (p.status === "done") {
    setFilePhase("done");
    showToast("Transcription complete");
    filePill.classList.remove("is-transcribing");
    setStatus("");
  } else if (p.status === "error") {
    setFilePhase("error");
    filePill.classList.remove("is-transcribing");
    setStatus(msg);
  } else if (p.status === "cancelled") {
    setFilePhase("idle");
    filePill.classList.remove("is-transcribing");
    setStatus(msg || "Cancelled");
  }
}).catch(() => {});
