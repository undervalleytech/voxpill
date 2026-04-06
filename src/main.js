import "./contextMenuGuard.js";
import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import { ask, message } from "@tauri-apps/plugin-dialog";
import { openUrl } from "@tauri-apps/plugin-opener";
import { relaunch } from "@tauri-apps/plugin-process";
import { check } from "@tauri-apps/plugin-updater";
import { LANG_OPTIONS, TRANSLATE_TARGET_OPTIONS } from "./locales.js";

const shell = document.getElementById("spotlight-shell");
const modelLine = document.getElementById("model-line");
const modelRows = document.getElementById("model-rows");
const chkStatusSounds = document.getElementById("chk-status-sounds");
const pttShortcutBtn = document.getElementById("ptt-shortcut-btn");
const fileShortcutBtn = document.getElementById("file-shortcut-btn");
const aiAssistShortcutBtn = document.getElementById("ai-assist-shortcut-btn");
const cycleProfileShortcutBtn = document.getElementById("cycle-profile-shortcut-btn");
const infoKbdDictation = document.getElementById("info-kbd-dictation");
const infoKbdTranscribeFile = document.getElementById("info-kbd-transcribe-file");
const infoKbdCycleProfile = document.getElementById("info-kbd-cycle-profile");
const infoKbdAiAssist = document.getElementById("info-kbd-ai-assist");
const gpuVerifyBadge = document.getElementById("gpu-verify-badge");
const appVersionLabel = document.getElementById("app-version-label");
const btnCheckUpdates = document.getElementById("btn-check-updates");
const sttGroqApiKeyInput = document.getElementById("stt-groq-api-key");
const btnSttGroqApiKeyToggle = document.getElementById("btn-stt-groq-api-key-toggle");
const reformApiKeyInput = document.getElementById("reform-api-key");
const btnReformApiKeyToggle = document.getElementById("btn-reform-api-key-toggle");
const reformProfilesRoot = document.getElementById("reform-profiles");
const btnAddReformProfile = document.getElementById("btn-add-reform-profile");

const INSERT_OPTIONS = [
  { value: "paste", label: "Paste into active app (Ctrl+V)" },
  { value: "clipboard", label: "Clipboard only" },
];
const DECODE_PROFILE_OPTIONS = [
  { value: "fast", label: "Fast" },
  { value: "balanced", label: "Balanced" },
  { value: "accurate", label: "Accurate" },
];
const STT_MODE_OPTIONS = [
  { value: "local", label: "Local Whisper" },
  { value: "cloud-groq", label: "Cloud Groq" },
];
const STT_CLOUD_MODEL_OPTIONS = [
  { value: "whisper-large-v3-turbo", label: "Whisper Large v3 Turbo" },
  { value: "whisper-large-v3", label: "Whisper Large v3" },
];

const REFORM_PROVIDER_OPTIONS = [
  { value: "groq", label: "Groq" },
  { value: "openrouter", label: "OpenRouter" },
];

const TIER_LABELS = {
  tiny: { title: "Tiny", sub: "Fastest, lower accuracy" },
  base: { title: "Base", sub: "Balanced speed and quality" },
  small: { title: "Small", sub: "Slower, better accuracy (CPU)" },
  medium: { title: "Medium", sub: "GPU recommended, higher quality" },
  "large-v3-turbo": { title: "Large v3 Turbo", sub: "Best quality, needs strong GPU/RAM" },
};

const CHECK_SVG = `<svg viewBox="0 0 16 16" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M3.5 8.2 6.3 11 12.5 4.8" stroke="rgba(255,255,255,0.9)" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

/** Icône téléchargement (flèche vers le bas). */
const DOWNLOAD_SVG = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M12 3v12m0 0l4-4m-4 4l-4-4M4 17v2a1 1 0 001 1h14a1 1 0 001-1v-2" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

const TRASH_SVG = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 002 2h6a2 2 0 002-2l1-12M9 7V5a1 1 0 011-1h4a1 1 0 011 1v2" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

/** Icônes compactes pour le menu profil (ordre dans la liste). */
const REFORM_LI_SVG_UP = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M12 19V5M5 12l7-7 7 7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const REFORM_LI_SVG_DOWN = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M12 5v14M19 12l-7 7-7-7" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"/></svg>`;
const REFORM_LI_SVG_TRASH = `<svg viewBox="0 0 24 24" fill="none" xmlns="http://www.w3.org/2000/svg" aria-hidden="true"><path d="M4 7h16M10 11v6M14 11v6M6 7l1 12a2 2 0 002 2h6a2 2 0 002-2l1-12M9 7V5a1 1 0 011-1h4a1 1 0 011 1v2" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"/></svg>`;

function escapeHtml(s) {
  return String(s)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

/** Téléchargement en cours (un seul à la fois). */
let activeDownloadTier = null;
/** Évite les écouteurs document dupliqués pour les custom-select. */
let customSelectDocumentListenersBound = false;
const DEFAULT_PTT_SHORTCUT = "Ctrl+Space";
const DEFAULT_FILE_SHORTCUT = "Ctrl+Shift+T";
const DEFAULT_AI_ASSIST_SHORTCUT = "Ctrl+Alt+Space";
const DEFAULT_CYCLE_PROFILE_SHORTCUT = "Ctrl+Alt+Shift+P";

let currentPttShortcut = DEFAULT_PTT_SHORTCUT;
let currentFileShortcut = DEFAULT_FILE_SHORTCUT;
let currentAiAssistShortcut = DEFAULT_AI_ASSIST_SHORTCUT;
let currentCycleProfileShortcut = DEFAULT_CYCLE_PROFILE_SHORTCUT;
/** @type {null | "ptt" | "file" | "assist" | "cycle"} */
let shortcutCaptureKind = null;

function prettyTier(tier) {
  return TIER_LABELS[tier]?.title ?? tier;
}

function openAnim() {
  shell?.classList.add("open");
}

function initSettingsTabs() {
  const navButtons = Array.from(document.querySelectorAll(".settings-nav-item[data-panel]"));
  if (!navButtons.length) return;
  const panels = Array.from(document.querySelectorAll(".settings-panel"));
  const activate = (panelId) => {
    navButtons.forEach((btn) => {
      btn.classList.toggle("is-active", btn.dataset.panel === panelId);
    });
    panels.forEach((panel) => {
      panel.classList.toggle("is-active", panel.id === panelId);
    });
  };
  navButtons.forEach((btn) => {
    btn.addEventListener("click", () => {
      activate(btn.dataset.panel);
    });
  });
  activate(navButtons[0].dataset.panel);
}

function normalizeShortcutDisplay(shortcut, defaultShortcut = DEFAULT_PTT_SHORTCUT) {
  return shortcut && shortcut.length ? shortcut : defaultShortcut;
}

function renderShortcutKbdGroup(el, shortcut) {
  if (!el) return;
  const s = String(shortcut || "").trim();
  el.replaceChildren();
  if (!s.length) return;
  const parts = s.split("+").map((p) => p.trim()).filter(Boolean);
  for (let i = 0; i < parts.length; i += 1) {
    if (i > 0) {
      const sep = document.createElement("span");
      sep.textContent = "+";
      el.appendChild(sep);
    }
    const kbd = document.createElement("kbd");
    kbd.textContent = parts[i];
    el.appendChild(kbd);
  }
}

function renderInfoShortcutKbds() {
  renderShortcutKbdGroup(infoKbdDictation, normalizeShortcutDisplay(currentPttShortcut, DEFAULT_PTT_SHORTCUT));
  renderShortcutKbdGroup(
    infoKbdTranscribeFile,
    normalizeShortcutDisplay(currentFileShortcut, DEFAULT_FILE_SHORTCUT)
  );
  renderShortcutKbdGroup(
    infoKbdCycleProfile,
    normalizeShortcutDisplay(currentCycleProfileShortcut, DEFAULT_CYCLE_PROFILE_SHORTCUT)
  );
  renderShortcutKbdGroup(
    infoKbdAiAssist,
    normalizeShortcutDisplay(currentAiAssistShortcut, DEFAULT_AI_ASSIST_SHORTCUT)
  );
}

function keyToShortcutToken(e) {
  const k = e.key;
  if (!k) return null;
  if (k === " ") return "Space";
  if (k === "Escape") return "Esc";
  if (k.startsWith("Arrow")) return k.replace("Arrow", "");
  if (k.length === 1) return k.toUpperCase();
  if (k === "Control" || k === "Shift" || k === "Alt" || k === "Meta") return null;
  return k;
}

function eventToShortcut(e) {
  const mods = [];
  if (e.ctrlKey) mods.push("Ctrl");
  if (e.shiftKey) mods.push("Shift");
  if (e.altKey) mods.push("Alt");
  if (e.metaKey) mods.push("Super");
  const keyToken = keyToShortcutToken(e);
  if (!keyToken) return null;
  return [...mods, keyToken].join("+");
}

function renderShortcutCaptureButtons() {
  const rows = [
    { kind: "ptt", btn: pttShortcutBtn, value: currentPttShortcut, def: DEFAULT_PTT_SHORTCUT },
    { kind: "file", btn: fileShortcutBtn, value: currentFileShortcut, def: DEFAULT_FILE_SHORTCUT },
    { kind: "assist", btn: aiAssistShortcutBtn, value: currentAiAssistShortcut, def: DEFAULT_AI_ASSIST_SHORTCUT },
    { kind: "cycle", btn: cycleProfileShortcutBtn, value: currentCycleProfileShortcut, def: DEFAULT_CYCLE_PROFILE_SHORTCUT },
  ];
  for (const { kind, btn, value, def } of rows) {
    if (!btn) continue;
    const capturing = shortcutCaptureKind === kind;
    btn.textContent = capturing ? "Press shortcut..." : normalizeShortcutDisplay(value, def);
    btn.classList.toggle("is-capturing", capturing);
  }
}

function initShortcutCapture() {
  const bindings = [
    {
      kind: "ptt",
      btn: pttShortcutBtn,
      get: () => currentPttShortcut,
      set: (v) => {
        currentPttShortcut = v;
      },
    },
    {
      kind: "file",
      btn: fileShortcutBtn,
      get: () => currentFileShortcut,
      set: (v) => {
        currentFileShortcut = v;
      },
    },
    {
      kind: "assist",
      btn: aiAssistShortcutBtn,
      get: () => currentAiAssistShortcut,
      set: (v) => {
        currentAiAssistShortcut = v;
      },
    },
    {
      kind: "cycle",
      btn: cycleProfileShortcutBtn,
      get: () => currentCycleProfileShortcut,
      set: (v) => {
        currentCycleProfileShortcut = v;
      },
    },
  ];
  renderShortcutCaptureButtons();
  renderInfoShortcutKbds();
  for (const { kind, btn, get, set } of bindings) {
    if (!btn) continue;
    btn.addEventListener("click", () => {
      shortcutCaptureKind = kind;
      renderShortcutCaptureButtons();
      btn.focus();
    });
    btn.addEventListener("keydown", (e) => {
      if (shortcutCaptureKind !== kind) return;
      e.preventDefault();
      e.stopPropagation();
      if (e.key === "Escape") {
        shortcutCaptureKind = null;
        renderShortcutCaptureButtons();
        return;
      }
      const shot = eventToShortcut(e);
      if (!shot) return;
      set(shot);
      shortcutCaptureKind = null;
      renderShortcutCaptureButtons();
      renderInfoShortcutKbds();
      void saveSettings();
    });
    btn.addEventListener("blur", () => {
      if (shortcutCaptureKind !== kind) return;
      shortcutCaptureKind = null;
      renderShortcutCaptureButtons();
    });
  }
}

function getRowDlElements(tier) {
  const wrap = modelRows
    ? Array.from(modelRows.querySelectorAll(".model-row-wrap")).find((w) => w.dataset.tier === tier)
    : null;
  if (!wrap) return null;
  const bar = wrap.querySelector(".model-row-dl-bar");
  const fill = bar?.querySelector(".dl-progress-fill");
  const track = bar?.querySelector(".dl-progress-track");
  const label = bar?.querySelector(".dl-progress-label");
  const btn = wrap.querySelector(".model-row-dl");
  return { wrap, bar, fill, track, label, btn };
}

function resetRowDlBar(tier) {
  const el = getRowDlElements(tier);
  if (!el?.bar || !el.fill || !el.track || !el.label) return;
  el.bar.hidden = true;
  el.fill.style.width = "0%";
  el.track.classList.remove("is-indeterminate");
  el.label.textContent = "";
}

function showRowDlBar(tier, show) {
  const el = getRowDlElements(tier);
  if (!el?.bar) return;
  el.bar.hidden = !show;
  if (!show) {
    el.fill.style.width = "0%";
    el.track.classList.remove("is-indeterminate");
    el.label.textContent = "";
  }
}

function buildLangAndTranslateLists() {
  const langUl = document.querySelector("#custom-lang .custom-select-list");
  if (langUl && !langUl.dataset.built) {
    langUl.innerHTML = LANG_OPTIONS.map(
      (o) => `<li role="option" data-value="${o.value}">${o.label}</li>`
    ).join("");
    langUl.dataset.built = "1";
  }
  document.querySelectorAll(".reform-profile-translate-list").forEach((trUl) => {
    if (trUl.dataset.built) return;
    trUl.innerHTML = TRANSLATE_TARGET_OPTIONS.map(
      (o) => `<li role="option" data-value="${o.value}">${o.label}</li>`
    ).join("");
    trUl.dataset.built = "1";
  });
}

function optionsForSelect(rootId) {
  if (!rootId) return [];
  if (rootId === "custom-lang") return LANG_OPTIONS;
  if (rootId === "custom-insert") return INSERT_OPTIONS;
  if (rootId === "custom-stt-mode") return STT_MODE_OPTIONS;
  if (rootId === "custom-stt-cloud-model") return STT_CLOUD_MODEL_OPTIONS;
  if (rootId.startsWith("custom-reform-p") && rootId.includes("-decode")) return DECODE_PROFILE_OPTIONS;
  if (rootId.startsWith("custom-reform-p") && rootId.includes("-translate-target")) {
    return TRANSLATE_TARGET_OPTIONS;
  }
  if (rootId === "custom-reformulation-provider") return REFORM_PROVIDER_OPTIONS;
  if (rootId === "custom-reformulation-active-profile") {
    const lis = document.querySelectorAll("#custom-reformulation-active-profile .custom-select-list li[data-value]");
    return Array.from(lis).map((li) => {
      const labelEl = li.querySelector(".reform-profile-option-label");
      const label = labelEl?.textContent?.trim() ?? li.dataset.value;
      return { value: li.dataset.value, label };
    });
  }
  return [];
}

function profileNameOrFallback(name, idx1) {
  const n = typeof name === "string" ? name.trim() : "";
  return n.length ? n : `Profile ${idx1}`;
}

function getReformProfileOptionsFromCfg(cfg) {
  const profiles = Array.isArray(cfg?.reformulation_profiles) ? cfg.reformulation_profiles : [];
  if (!profiles.length) {
    return [{ value: "", label: "Profile 1" }];
  }
  return profiles.map((p, idx) => ({
    value: typeof p.id === "string" && p.id.length ? p.id : `__pending_${idx}`,
    label: profileNameOrFallback(p?.profile_name, idx + 1),
  }));
}

function rebuildReformProfileSelect(options) {
  const ul = document.querySelector("#custom-reformulation-active-profile .custom-select-list");
  if (!ul) return;
  const n = options.length;
  ul.innerHTML = options
    .map((o, idx) => {
      const disUp = idx <= 0 || n <= 1;
      const disDown = idx >= n - 1 || n <= 1;
      const disRem = n <= 1;
      const v = escapeHtml(o.value);
      const lab = escapeHtml(o.label);
      return `<li role="option" data-value="${v}" class="reform-profile-option">
  <span class="reform-profile-option-label">${lab}</span>
  <span class="reform-profile-option-actions">
    <button type="button" class="reform-profile-li-btn" data-action="up" data-profile-id="${v}" ${disUp ? "disabled" : ""} title="Move up in list" aria-label="Move profile up">${
      REFORM_LI_SVG_UP
    }</button>
    <button type="button" class="reform-profile-li-btn" data-action="down" data-profile-id="${v}" ${disDown ? "disabled" : ""} title="Move down in list" aria-label="Move profile down">${
      REFORM_LI_SVG_DOWN
    }</button>
    <button type="button" class="reform-profile-li-btn reform-profile-li-btn-danger" data-action="remove" data-profile-id="${v}" ${disRem ? "disabled" : ""} title="Remove profile" aria-label="Remove profile">${
      REFORM_LI_SVG_TRASH
    }</button>
  </span>
</li>`;
    })
    .join("");
}

function reformulationFromDom() {
  const blocks = Array.from(document.querySelectorAll("#reform-profiles .reform-profile-block"));
  const profiles = [];
  for (let i = 0; i < blocks.length; i += 1) {
    const slot = i + 1;
    const block = blocks[i];
    const profileId = (block.dataset.profileId ?? "").trim();
    const decodeRaw = document.getElementById(`custom-reform-p${slot}-decode`)?.dataset.value ?? "balanced";
    const decode_profile = ["fast", "balanced", "accurate"].includes(decodeRaw) ? decodeRaw : "balanced";
    const translate_enabled = document.getElementById(`chk-reform-p${slot}-translate`)?.checked ?? false;
    const translate_target =
      document.getElementById(`custom-reform-p${slot}-translate-target`)?.dataset.value ?? "en";
    const reformulation_enabled = document.getElementById(`chk-reform-p${slot}-reformulation`)?.checked ?? false;
    profiles.push({
      id: profileId,
      profile_name: document.getElementById(`reform-p${slot}-name`)?.value?.trim() ?? "",
      model: document.getElementById(`reform-p${slot}-model`)?.value?.trim() ?? "",
      prompt: document.getElementById(`reform-p${slot}-prompt`)?.value ?? "",
      decode_profile,
      translate_enabled,
      translate_target,
      reformulation_enabled,
    });
  }
  const apRaw = document.getElementById("custom-reformulation-active-profile")?.dataset.value?.trim() ?? "";
  const idSet = new Set(profiles.map((p) => p.id).filter((x) => x.length > 0));
  let reformulation_active_profile_id = apRaw;
  if (!reformulation_active_profile_id || !idSet.has(reformulation_active_profile_id)) {
    reformulation_active_profile_id = profiles[0]?.id ?? "";
  }
  const active = profiles.find((p) => p.id === reformulation_active_profile_id) ?? profiles[0];
  return {
    reformulation_enabled: active?.reformulation_enabled ?? false,
    decode_profile: active?.decode_profile ?? "balanced",
    translate_enabled: active?.translate_enabled ?? false,
    translate_target: active?.translate_target ?? "en",
    reformulation_api_key: document.getElementById("reform-api-key")?.value ?? "",
    reformulation_provider: document.getElementById("custom-reformulation-provider")?.dataset.value ?? "groq",
    reformulation_active_profile_id,
    reformulation_active_profile: null,
    reformulation_profiles: profiles,
    reformulation_assist_model: document.getElementById("reform-assist-model")?.value?.trim() ?? "",
  };
}

/** Show only the profile block matching the currently selected profile. */
function updateReformProfilePanelVisibility() {
  const activeId = document.getElementById("custom-reformulation-active-profile")?.dataset.value?.trim() ?? "";
  document.querySelectorAll("#reform-profiles .reform-profile-block").forEach((el) => {
    const pid = (el.dataset.profileId ?? "").trim();
    el.classList.toggle("is-visible", pid.length > 0 && pid === activeId);
  });
}

/** Reconstruit les blocs profil (slots 1..n) pour garder des ids de champs cohérents après suppression. */
function rebuildReformProfileBlocks(count) {
  if (!reformProfilesRoot) return;
  reformProfilesRoot.innerHTML = "";
  for (let i = 1; i <= count; i += 1) {
    reformProfilesRoot.insertAdjacentHTML("beforeend", createReformProfileBlockHtml(i));
  }
}

function createReformProfileBlockHtml(i) {
  return `<div class="reform-profile-block" data-profile="${i}" data-profile-id="">
    <div class="field">
      <span>Profile name</span>
      <input type="text" id="reform-p${i}-name" class="reform-text-input" spellcheck="false" placeholder="e.g. Profile ${i}" />
    </div>
    <div class="field">
      <span>Decoding profile</span>
      <div class="custom-select" id="custom-reform-p${i}-decode" data-value="balanced">
        <button type="button" class="custom-select-trigger" aria-expanded="false" aria-haspopup="listbox">
          <span class="custom-select-value">Balanced</span>
          <span class="custom-select-chevron" aria-hidden="true"></span>
        </button>
        <ul class="custom-select-list" role="listbox" hidden>
          <li role="option" data-value="fast">Fast</li>
          <li role="option" data-value="balanced">Balanced</li>
          <li role="option" data-value="accurate">Accurate</li>
        </ul>
      </div>
    </div>
    <div class="field">
      <span>Translation</span>
      <label class="toggle-row">
        <input type="checkbox" id="chk-reform-p${i}-translate" />
        <span class="toggle-label">Translate after dictation</span>
      </label>
    </div>
    <div class="field">
      <span>Target language</span>
      <div class="custom-select" id="custom-reform-p${i}-translate-target" data-value="en">
        <button type="button" class="custom-select-trigger" aria-expanded="false" aria-haspopup="listbox">
          <span class="custom-select-value">English</span>
          <span class="custom-select-chevron" aria-hidden="true"></span>
        </button>
        <ul class="custom-select-list reform-profile-translate-list" role="listbox" hidden></ul>
      </div>
    </div>
    <div class="field">
      <span id="label-reform-p${i}-reformulation">Enable AI reformulation</span>
      <label class="toggle-row">
        <input type="checkbox" id="chk-reform-p${i}-reformulation" class="reform-reformulation-chk" aria-labelledby="label-reform-p${i}-reformulation" />
      </label>
    </div>
    <div class="reform-llm-fields">
    <div class="field">
      <span>Model</span>
      <input type="text" id="reform-p${i}-model" class="reform-text-input" spellcheck="false" placeholder="e.g. llama-3.1-8b-instant" />
    </div>
    <div class="field field-stack">
      <span>Prompt</span>
      <div class="reform-prompt-wrap">
        <textarea id="reform-p${i}-prompt" class="reform-textarea" rows="8" spellcheck="false"></textarea>
        <button type="button" class="reform-prompt-assist-btn" data-profile="${i}" title="AI assistant for this prompt">Refine prompt</button>
      </div>
    </div>
    </div>
  </div>`;
}

async function reorderProfileById(profileId, delta) {
  const cfg = await invoke("get_config");
  const profiles = [...(cfg.reformulation_profiles ?? [])];
  const i = profiles.findIndex((p) => p.id === profileId);
  if (i < 0) return;
  const j = i + delta;
  if (j < 0 || j >= profiles.length) return;
  const next = [...profiles];
  [next[i], next[j]] = [next[j], next[i]];
  await invoke("set_config", {
    cfg: {
      ...cfg,
      reformulation_profiles: next,
      reformulation_active_profile: null,
    },
  });
  await refresh();
}

async function removeProfileById(profileId) {
  const cfg = await invoke("get_config");
  const profiles = [...(cfg.reformulation_profiles ?? [])];
  if (profiles.length <= 1) return;
  const i = profiles.findIndex((p) => p.id === profileId);
  if (i < 0) return;
  const removed = profiles[i];
  const activeId = cfg.reformulation_active_profile_id;
  const next = profiles.filter((_, idx) => idx !== i);
  let newActiveId = activeId;
  if (removed?.id && removed.id === activeId) {
    newActiveId = next[Math.max(0, i - 1)]?.id ?? next[0]?.id ?? "";
  }
  await invoke("set_config", {
    cfg: {
      ...cfg,
      reformulation_profiles: next,
      reformulation_active_profile_id: newActiveId,
      reformulation_active_profile: null,
    },
  });
  await refresh();
}

async function assistPromptForProfile(profileId, btn) {
  const provider = document.getElementById("custom-reformulation-provider")?.dataset.value ?? "groq";
  const apiKey = reformApiKeyInput?.value?.trim() ?? "";
  const promptEl = document.getElementById(`reform-p${profileId}-prompt`);
  const modelEl = document.getElementById(`reform-p${profileId}-model`);
  if (!promptEl) return;
  const draft = promptEl.value?.trim() ?? "";
  if (!apiKey) {
    alert("Add your API key first in AI setup.");
    return;
  }
  if (!draft) {
    alert("Write a draft in Prompt first, then click Refine prompt.");
    return;
  }
  const original = btn?.textContent ?? "Refine prompt";
  if (btn) {
    btn.disabled = true;
    btn.textContent = "Refining...";
  }
  try {
    const refined = await invoke("assist_reformulation_prompt", {
      input: {
        provider,
        api_key: apiKey,
        draft,
        model_hint: modelEl?.value?.trim() ?? "",
      },
    });
    if (typeof refined === "string" && refined.trim().length) {
      promptEl.value = refined.trim();
      await saveSettings();
    }
  } catch (e) {
    alert(String(e));
  } finally {
    if (btn) {
      btn.disabled = false;
      btn.textContent = original;
    }
  }
}

function setCustomSelect(rootId, value, options) {
  const root = document.getElementById(rootId);
  if (!root) return;
  const opt = options.find((o) => o.value === value);
  root.dataset.value = value;
  const valEl = root.querySelector(".custom-select-value");
  if (valEl) valEl.textContent = opt?.label ?? value;
  root.querySelectorAll("li[role=option]").forEach((li) => {
    li.setAttribute("aria-selected", li.dataset.value === value ? "true" : "false");
  });
}

function closeAllCustomSelects() {
  document.querySelectorAll(".custom-select-list").forEach((list) => {
    list.hidden = true;
    list.closest(".custom-select")?.querySelector(".custom-select-trigger")?.setAttribute("aria-expanded", "false");
  });
}

function cloudModeEnabled() {
  return (document.getElementById("custom-stt-mode")?.dataset.value ?? "local") === "cloud-groq";
}

function cloudGroqKeyPresent() {
  return (sttGroqApiKeyInput?.value?.trim() ?? "").length > 0;
}

function updateSttCloudUiState() {
  const cloudOn = cloudModeEnabled();
  const hasKey = cloudGroqKeyPresent();
  const modelField = document.getElementById("stt-cloud-model-field");
  const keyField = document.getElementById("stt-cloud-key-field");
  const help = document.getElementById("stt-cloud-help");
  const modelTrigger = document.querySelector("#custom-stt-cloud-model .custom-select-trigger");

  modelField?.classList.toggle("is-disabled", !cloudOn);
  keyField?.classList.toggle("is-disabled", !cloudOn);
  if (modelTrigger) modelTrigger.disabled = !cloudOn;

  if (help) {
    if (!cloudOn) {
      help.textContent = "Cloud STT is disabled. Local Whisper remains active.";
    } else if (!hasKey) {
      help.textContent = "Add a valid Groq API key to use Cloud STT. Otherwise local fallback will be used.";
    } else {
      help.textContent = "Cloud STT active with Groq. If cloud fails, Voxpill falls back to local Whisper.";
    }
  }
}

function initCustomSelects() {
  document.querySelectorAll(".custom-select").forEach((root) => {
    if (root.dataset.vpBound === "1") return;
    root.dataset.vpBound = "1";
    const trigger = root.querySelector(".custom-select-trigger");
    const list = root.querySelector(".custom-select-list");
    if (!trigger || !list) return;
    trigger.addEventListener("click", (e) => {
      e.stopPropagation();
      const willOpen = list.hidden;
      closeAllCustomSelects();
      if (willOpen) {
        list.hidden = false;
        trigger.setAttribute("aria-expanded", "true");
      }
    });
    list.addEventListener("click", (e) => {
      if (root.id === "custom-reformulation-active-profile") {
        const actBtn = e.target.closest(".reform-profile-li-btn");
        if (actBtn) {
          e.stopPropagation();
          e.preventDefault();
          if (actBtn.disabled) return;
          const id = actBtn.dataset.profileId?.trim() ?? "";
          const action = actBtn.dataset.action;
          if (!id || !action) return;
          if (action === "up") void reorderProfileById(id, -1);
          else if (action === "down") void reorderProfileById(id, 1);
          else if (action === "remove") void removeProfileById(id);
          return;
        }
      }
      const li = e.target.closest("li[data-value]");
      if (!li) return;
      e.stopPropagation();
      const value = li.dataset.value;
      const opts = optionsForSelect(root.id);
      setCustomSelect(root.id, value, opts);
      if (root.id === "custom-reformulation-active-profile") {
        updateReformProfilePanelVisibility();
      }
      if (root.id === "custom-stt-mode") {
        updateSttCloudUiState();
      }
      list.hidden = true;
      trigger.setAttribute("aria-expanded", "false");
      void saveSettings();
    });
  });
  if (!customSelectDocumentListenersBound) {
    customSelectDocumentListenersBound = true;
    document.addEventListener("click", () => {
      closeAllCustomSelects();
    });
    document.addEventListener("keydown", (e) => {
      if (shortcutCaptureKind) return;
      if (e.key === "Escape") closeAllCustomSelects();
    });
  }
}

function renderModelRows(overview) {
  if (!modelRows) return;
  modelRows.innerHTML = overview.tiers
    .map((t) => {
      const sel = overview.selected_tier === t.tier;
      const meta = t.exists ? "Downloaded" : "Not downloaded";
      const L = TIER_LABELS[t.tier] ?? { title: t.tier, sub: "" };
      const sub = L.sub ? ` · ${L.sub}` : "";
      const trail = t.exists
        ? `<div class="model-row-trail-actions">
  <span class="model-row-badge" title="File available on this PC">${CHECK_SVG}</span>
  <button type="button" class="model-row-del" data-tier="${t.tier}" aria-label="Delete ${L.title}" title="Delete from disk">${TRASH_SVG}</button>
</div>`
        : `<button type="button" class="model-row-dl" data-tier="${t.tier}" aria-label="Download ${L.title}" title="Download">${DOWNLOAD_SVG}</button>`;
      return `<div class="model-row-wrap${sel ? " is-selected" : ""}" data-tier="${t.tier}">
  <label class="model-row${sel ? " is-selected" : ""}">
    <input type="radio" name="model-tier" value="${t.tier}" ${sel ? "checked" : ""} />
    <span class="model-radio-ui" aria-hidden="true"></span>
    <span class="model-row-body">
      <span class="model-row-title">${L.title}</span>
      <span class="model-row-meta">${meta}${sub}</span>
    </span>
  </label>
  <div class="model-row-trail">${trail}</div>
  <div class="model-row-dl-bar" data-tier="${t.tier}" hidden>
    <div class="dl-progress-track">
      <div class="dl-progress-fill" style="width: 0%"></div>
    </div>
    <p class="dl-progress-label"></p>
  </div>
</div>`;
    })
    .join("");
  setDownloadButtonsDisabled();
}

function setDownloadButtonsDisabled() {
  if (!modelRows) return;
  const busy = activeDownloadTier !== null;
  modelRows.querySelectorAll(".model-row-dl").forEach((btn) => {
    btn.disabled = busy && btn.dataset.tier !== activeDownloadTier;
  });
  modelRows.querySelectorAll(".model-row-del").forEach((btn) => {
    btn.disabled = busy;
  });
}

function updateModelLine(overview, cfg = null) {
  const cur = overview.tiers.find((x) => x.tier === overview.selected_tier);
  const sttMode = cfg?.stt_mode === "cloud-groq" ? "cloud-groq" : "local";
  if (sttMode === "cloud-groq") {
    const cloudModelRaw = typeof cfg?.stt_cloud_model === "string" ? cfg.stt_cloud_model : "whisper-large-v3-turbo";
    const cloudLabel = STT_CLOUD_MODEL_OPTIONS.find((o) => o.value === cloudModelRaw)?.label ?? cloudModelRaw;
    modelLine.textContent = `Cloud STT · ${cloudLabel} (Groq)`;
    return;
  }
  if (!cur) {
    modelLine.textContent = `Local Whisper · ${prettyTier(overview.selected_tier)} selected`;
    return;
  }
  if (cur.exists) {
    modelLine.textContent = `Local Whisper · ${prettyTier(overview.selected_tier)} selected`;
  } else {
    modelLine.textContent = `Local Whisper · ${prettyTier(overview.selected_tier)} · not downloaded`;
  }
}

async function refresh() {
  const hw = await invoke("get_hardware_profile");
  if (gpuVerifyBadge) gpuVerifyBadge.hidden = !Boolean(hw.whisper_gpu);

  const overview = await invoke("models_overview");
  renderModelRows(overview);
  updateModelLine(overview);

  const cfg = await invoke("get_config");
  const lang = cfg.language === "auto" ? "auto" : cfg.language;
  setCustomSelect("custom-lang", lang, LANG_OPTIONS);
  const ins = cfg.insert_mode === "clipboard" ? "clipboard" : "paste";
  setCustomSelect("custom-insert", ins, INSERT_OPTIONS);
  const sttModeRaw = typeof cfg.stt_mode === "string" ? cfg.stt_mode : "local";
  const sttMode = ["local", "cloud-groq"].includes(sttModeRaw) ? sttModeRaw : "local";
  setCustomSelect("custom-stt-mode", sttMode, STT_MODE_OPTIONS);
  const sttModelRaw =
    typeof cfg.stt_cloud_model === "string" ? cfg.stt_cloud_model : "whisper-large-v3-turbo";
  const sttModel = ["whisper-large-v3", "whisper-large-v3-turbo"].includes(sttModelRaw)
    ? sttModelRaw
    : "whisper-large-v3-turbo";
  setCustomSelect("custom-stt-cloud-model", sttModel, STT_CLOUD_MODEL_OPTIONS);
  if (sttGroqApiKeyInput) {
    sttGroqApiKeyInput.value = typeof cfg.stt_groq_api_key === "string" ? cfg.stt_groq_api_key : "";
  }
  updateSttCloudUiState();
  if (chkStatusSounds) chkStatusSounds.checked = cfg.status_sounds_enabled !== false;
  currentPttShortcut =
    typeof cfg.ptt_shortcut === "string" && cfg.ptt_shortcut.length ? cfg.ptt_shortcut : DEFAULT_PTT_SHORTCUT;
  currentFileShortcut =
    typeof cfg.shortcut_transcribe_file === "string" && cfg.shortcut_transcribe_file.length
      ? cfg.shortcut_transcribe_file
      : DEFAULT_FILE_SHORTCUT;
  currentAiAssistShortcut =
    typeof cfg.shortcut_ai_assist === "string" && cfg.shortcut_ai_assist.length
      ? cfg.shortcut_ai_assist
      : DEFAULT_AI_ASSIST_SHORTCUT;
  currentCycleProfileShortcut =
    typeof cfg.shortcut_cycle_profile === "string" && cfg.shortcut_cycle_profile.length
      ? cfg.shortcut_cycle_profile
      : DEFAULT_CYCLE_PROFILE_SHORTCUT;
  renderShortcutCaptureButtons();
  renderInfoShortcutKbds();
  const provRaw = typeof cfg.reformulation_provider === "string" ? cfg.reformulation_provider : "groq";
  const prov = ["groq", "openrouter"].includes(provRaw) ? provRaw : "groq";
  setCustomSelect("custom-reformulation-provider", prov, REFORM_PROVIDER_OPTIONS);
  if (reformApiKeyInput) reformApiKeyInput.value = typeof cfg.reformulation_api_key === "string" ? cfg.reformulation_api_key : "";
  const profiles = Array.isArray(cfg.reformulation_profiles) ? cfg.reformulation_profiles : [];
  rebuildReformProfileBlocks(profiles.length);
  const profileOptions = getReformProfileOptionsFromCfg(cfg);
  rebuildReformProfileSelect(profileOptions);
  let activeId = typeof cfg.reformulation_active_profile_id === "string" ? cfg.reformulation_active_profile_id.trim() : "";
  const idsFromCfg = new Set(profiles.map((p) => p.id).filter((x) => typeof x === "string" && x.length > 0));
  if (!activeId || !idsFromCfg.has(activeId)) {
    activeId = profiles[0]?.id ?? "";
  }
  setCustomSelect("custom-reformulation-active-profile", activeId, profileOptions);
  buildLangAndTranslateLists();
  for (let i = 1; i <= profiles.length; i += 1) {
    const p = profiles[i - 1];
    const block = reformProfilesRoot?.querySelector(`.reform-profile-block[data-profile="${i}"]`);
    if (block && typeof p?.id === "string" && p.id.length) block.dataset.profileId = p.id;
    const nameEl = document.getElementById(`reform-p${i}-name`);
    const modelEl = document.getElementById(`reform-p${i}-model`);
    const promptEl = document.getElementById(`reform-p${i}-prompt`);
    if (nameEl) nameEl.value = profileNameOrFallback(p?.profile_name, i);
    if (modelEl) modelEl.value = p && typeof p.model === "string" ? p.model : "";
    if (promptEl) promptEl.value = p && typeof p.prompt === "string" ? p.prompt : "";
    const dpRaw = typeof p?.decode_profile === "string" ? p.decode_profile : "balanced";
    const dp = ["fast", "balanced", "accurate"].includes(dpRaw) ? dpRaw : "balanced";
    setCustomSelect(`custom-reform-p${i}-decode`, dp, DECODE_PROFILE_OPTIONS);
    const trEl = document.getElementById(`chk-reform-p${i}-translate`);
    if (trEl) trEl.checked = Boolean(p?.translate_enabled);
    const tt = typeof p?.translate_target === "string" && p.translate_target.length ? p.translate_target : "en";
    setCustomSelect(`custom-reform-p${i}-translate-target`, tt, TRANSLATE_TARGET_OPTIONS);
    const reEl = document.getElementById(`chk-reform-p${i}-reformulation`);
    if (reEl) reEl.checked = Boolean(p?.reformulation_enabled);
  }
  const assistModelEl = document.getElementById("reform-assist-model");
  if (assistModelEl) {
    assistModelEl.value = typeof cfg.reformulation_assist_model === "string" ? cfg.reformulation_assist_model : "";
  }
  updateModelLine(overview, cfg);
  updateReformProfilePanelVisibility();
  initCustomSelects();
}

async function startDownload(tier) {
  if (activeDownloadTier) return;
  activeDownloadTier = tier;
  setDownloadButtonsDisabled();
  showRowDlBar(tier, true);
  const el = getRowDlElements(tier);
  if (el?.fill && el.track && el.label) {
    el.track.classList.remove("is-indeterminate");
    el.fill.style.width = "0%";
    el.label.textContent = "Connecting...";
  }
  try {
    await invoke("download_model", { tier });
  } catch (e) {
    console.error(e);
    modelLine.textContent = String(e);
    showRowDlBar(tier, false);
    activeDownloadTier = null;
    setDownloadButtonsDisabled();
    await refresh();
  }
}

modelRows?.addEventListener("click", (e) => {
  const delBtn = e.target.closest(".model-row-del");
  if (delBtn?.dataset?.tier) {
    e.preventDefault();
    e.stopPropagation();
    const tier = delBtn.dataset.tier;
    const name = prettyTier(tier);
    if (
      !confirm(`Delete "${name}" from this PC?\n\nYou can download it again later.`)
    ) {
      return;
    }
    void (async () => {
      try {
        await invoke("delete_model", { tier });
        await refresh();
      } catch (err) {
        console.error(err);
        modelLine.textContent = String(err);
      }
    })();
    return;
  }
  const btn = e.target.closest(".model-row-dl");
  if (!btn || !btn.dataset.tier) return;
  e.preventDefault();
  e.stopPropagation();
  void startDownload(btn.dataset.tier);
});

modelRows?.addEventListener("change", (e) => {
  const t = e.target;
  if (t?.name !== "model-tier") return;
  void saveModelTier(t.value);
});

listen("download-progress", (ev) => {
  const p = ev.payload;
  if (!p || typeof p !== "object") return;
  const tier = typeof p.tier === "string" ? p.tier : null;
  if (!tier) return;
  const el = getRowDlElements(tier);
  if (!el?.bar || !el.fill || !el.track || !el.label) return;
  el.bar.hidden = false;
  if (p.indeterminate) {
    el.track.classList.add("is-indeterminate");
    el.label.textContent = "Downloading...";
  } else {
    el.track.classList.remove("is-indeterminate");
    const pct = typeof p.pct === "number" ? p.pct : 0;
    el.fill.style.width = `${pct}%`;
    if (pct >= 100) {
      el.label.textContent = "Finishing...";
    } else {
      el.label.textContent = `Downloading: ${pct}%`;
    }
  }
});

listen("download-complete", (ev) => {
  const p = ev.payload;
  const tier = p && typeof p === "object" && typeof p.tier === "string" ? p.tier : activeDownloadTier;
  if (!tier) return;
  const el = getRowDlElements(tier);
  if (!el?.fill || !el.track || !el.label || !el.bar) return;
  el.bar.hidden = false;
  el.track.classList.remove("is-indeterminate");
  el.fill.style.width = "100%";
  const mb = p && typeof p === "object" && typeof p.size_mb === "number" ? p.size_mb : "";
  el.label.textContent =
    mb !== "" ? `Model ready (${mb} MB)` : "Model ready";
  window.setTimeout(() => {
    showRowDlBar(tier, false);
    activeDownloadTier = null;
    setDownloadButtonsDisabled();
    void refresh();
  }, 2000);
});

listen("model-ready", (ev) => {
  const p = ev.payload;
  if (p && typeof p === "object" && "tier" in p) {
    return;
  }
  activeDownloadTier = null;
  setDownloadButtonsDisabled();
  void refresh();
});

async function saveSettings() {
  const lang = document.getElementById("custom-lang")?.dataset.value ?? "fr";
  const insertRaw = document.getElementById("custom-insert")?.dataset.value ?? "paste";
  const insert_mode = insertRaw === "clipboard" ? "clipboard" : "paste";
  const sttModeRaw = document.getElementById("custom-stt-mode")?.dataset.value ?? "local";
  const stt_mode = sttModeRaw === "cloud-groq" ? "cloud-groq" : "local";
  const sttModelRaw =
    document.getElementById("custom-stt-cloud-model")?.dataset.value ?? "whisper-large-v3-turbo";
  const stt_cloud_model =
    sttModelRaw === "whisper-large-v3" || sttModelRaw === "whisper-large-v3-turbo"
      ? sttModelRaw
      : "whisper-large-v3-turbo";
  const ptt_shortcut = normalizeShortcutDisplay(currentPttShortcut, DEFAULT_PTT_SHORTCUT);
  const shortcut_transcribe_file = normalizeShortcutDisplay(currentFileShortcut, DEFAULT_FILE_SHORTCUT);
  const shortcut_ai_assist = normalizeShortcutDisplay(currentAiAssistShortcut, DEFAULT_AI_ASSIST_SHORTCUT);
  const shortcut_cycle_profile = normalizeShortcutDisplay(currentCycleProfileShortcut, DEFAULT_CYCLE_PROFILE_SHORTCUT);
  const cfg = await invoke("get_config");
  const rf = reformulationFromDom();
  await invoke("set_config", {
    cfg: {
      ...cfg,
      language: lang,
      insert_mode,
      model_tier: cfg.model_tier,
      stt_mode,
      stt_cloud_model,
      stt_groq_api_key: sttGroqApiKeyInput?.value?.trim() ?? "",
      ptt_shortcut,
      shortcut_transcribe_file,
      shortcut_ai_assist,
      shortcut_cycle_profile,
      status_sounds_enabled: chkStatusSounds?.checked ?? true,
      ...rf,
    },
  });
  await refresh();
}

async function saveModelTier(tier) {
  const cfg = await invoke("get_config");
  await invoke("set_config", {
    cfg: {
      ...cfg,
      model_tier: tier,
    },
  });
  await refresh();
}

sttGroqApiKeyInput?.addEventListener("blur", () => {
  void saveSettings();
});
chkStatusSounds?.addEventListener("change", () => {
  void saveSettings();
});
reformProfilesRoot?.addEventListener("focusout", (e) => {
  const t = e.target;
  if (!t) return;
  if (
    t.matches?.("[id^='reform-p'][id$='-name']") ||
    t.matches?.("[id^='reform-p'][id$='-model']") ||
    t.matches?.("[id^='reform-p'][id$='-prompt']")
  ) {
    void saveSettings();
  }
});
reformProfilesRoot?.addEventListener("click", (e) => {
  const btn = e.target.closest(".reform-prompt-assist-btn");
  if (!btn) return;
  const profile = parseInt(btn.dataset.profile ?? "1", 10);
  if (Number.isNaN(profile) || profile < 1) return;
  void assistPromptForProfile(profile, btn);
});
reformProfilesRoot?.addEventListener("change", (e) => {
  const t = e.target;
  if (!t) return;
  if (
    t.matches?.("[id^='chk-reform-p'][id$='-translate']") ||
    t.matches?.("[id^='chk-reform-p'][id$='-reformulation']")
  ) {
    void saveSettings();
  }
});

btnAddReformProfile?.addEventListener("click", async () => {
  const cfg = await invoke("get_config");
  const current = Array.isArray(cfg.reformulation_profiles) ? cfg.reformulation_profiles : [];
  const nextIndex = current.length + 1;
  const first = current[0];
  const newId = crypto.randomUUID();
  const next = {
    id: newId,
    profile_name: `Profile ${nextIndex}`,
    model: first?.model ?? "llama-3.1-8b-instant",
    prompt: first?.prompt ?? "",
    decode_profile: ["fast", "balanced", "accurate"].includes(first?.decode_profile)
      ? first.decode_profile
      : "balanced",
    translate_enabled: Boolean(first?.translate_enabled),
    translate_target:
      typeof first?.translate_target === "string" && first.translate_target.length
        ? first.translate_target
        : "en",
    reformulation_enabled: Boolean(first?.reformulation_enabled),
  };
  await invoke("set_config", {
    cfg: {
      ...cfg,
      reformulation_active_profile_id: newId,
      reformulation_active_profile: null,
      reformulation_profiles: [...current, next],
    },
  });
  await refresh();
});

reformApiKeyInput?.addEventListener("blur", () => {
  void saveSettings();
});
document.getElementById("reform-assist-model")?.addEventListener("blur", () => {
  void saveSettings();
});

let reformApiKeyVisible = false;
btnReformApiKeyToggle?.addEventListener("click", () => {
  reformApiKeyVisible = !reformApiKeyVisible;
  if (reformApiKeyInput) reformApiKeyInput.type = reformApiKeyVisible ? "text" : "password";
  if (btnReformApiKeyToggle) btnReformApiKeyToggle.textContent = reformApiKeyVisible ? "Hide" : "Show";
});
let sttGroqApiKeyVisible = false;
btnSttGroqApiKeyToggle?.addEventListener("click", () => {
  sttGroqApiKeyVisible = !sttGroqApiKeyVisible;
  if (sttGroqApiKeyInput) sttGroqApiKeyInput.type = sttGroqApiKeyVisible ? "text" : "password";
  if (btnSttGroqApiKeyToggle) btnSttGroqApiKeyToggle.textContent = sttGroqApiKeyVisible ? "Hide" : "Show";
});

listen("cloud-stt-status", (ev) => {
  const p = ev.payload;
  if (!p || typeof p !== "object") return;
  const message = typeof p.message === "string" ? p.message : "";
  if (!message) return;
  modelLine.textContent = message;
  window.setTimeout(() => {
    void refresh();
  }, 2500);
}).catch(() => {});

listen("palette-open", () => {
  openAnim();
  void refresh();
});

listen("profile-active-changed", () => {
  void refresh();
}).catch(() => {});

document.getElementById("window-drag-strip")?.addEventListener("mousedown", (e) => {
  if (e.button !== 0) return;
  void getCurrentWebviewWindow().startDragging().catch(() => {});
});
document.getElementById("close-settings-btn")?.addEventListener("click", () => {
  void getCurrentWebviewWindow().hide().catch(() => {});
});

async function refreshAppVersionLabel() {
  if (!appVersionLabel) return;
  try {
    const v = await getVersion();
    appVersionLabel.textContent = v ? `Version ${v}` : "";
  } catch {
    appVersionLabel.textContent = "";
  }
}

btnCheckUpdates?.addEventListener("click", () => {
  void (async () => {
    btnCheckUpdates.disabled = true;
    try {
      const update = await check();
      if (!update) {
        await message("You are using the latest version.", { title: "Updates" });
        return;
      }
      const notes = update.body?.trim() ? `\n\n${update.body}` : "";
      const ok = await ask(`Version ${update.version} is available.${notes}\n\nDownload and install now?`, {
        title: "Update available",
        kind: "info",
      });
      if (!ok) return;
      await update.downloadAndInstall();
      await message("Update installed. The application will restart.", { title: "Updates" });
      await relaunch();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      await message(`Update failed: ${msg}`, { title: "Updates", kind: "error" });
    } finally {
      btnCheckUpdates.disabled = false;
    }
  })();
});

document.querySelectorAll(".reform-key-links a[href]").forEach((a) => {
  a.addEventListener("click", (e) => {
    const href = a.getAttribute("href") ?? "";
    if (!/^https?:\/\//i.test(href)) return;
    e.preventDefault();
    void openUrl(href).catch((err) => {
      console.error("Failed to open external link:", err);
    });
  });
});

buildLangAndTranslateLists();
initCustomSelects();
initSettingsTabs();
initShortcutCapture();
updateReformProfilePanelVisibility();
void refresh();
void refreshAppVersionLabel();
openAnim();
