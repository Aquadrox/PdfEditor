// Démarrage de l'interface : assemble les modules, applique les réglages et
// gère le document ouvert. Un seul document à la fois en v0.1 (onglets : V-06).

import { attachMenu } from "./components/menu.js";
import { toast } from "./components/toast.js";
import { showHome, watchDrops } from "./features/home/home.js";
import { renderRecent } from "./features/home/recent.js";
import { openPath, pickPdf } from "./features/open/open.js";
import { attachSearch } from "./features/search/search-bar.js";
import { attachSettings } from "./features/settings/settings-view.js";
import { attachUpdates } from "./features/updates/updates.js";
import { renderOutline } from "./features/viewer/outline.js";
import { attachStatusbar } from "./features/viewer/statusbar.js";
import { Thumbnails } from "./features/viewer/thumbnails.js";
import { Viewer } from "./features/viewer/viewer.js";
import { zoomIn, zoomOut } from "./features/viewer/zoom.js";
import { byId } from "./lib/dom.js";
import { t, translate } from "./lib/i18n.js";
import { invoke, setWindowTitle } from "./lib/ipc.js";
import { eventToShortcut, keymap, shortcutLabel } from "./lib/shortcuts.js";
import { applyTheme } from "./lib/theme.js";

translate(document);

let current = null;
let settings = null;
let shortcuts = new Map();

const viewer = new Viewer(byId("desk"), {
  onPageChange: (page) => {
    statusbar.setPage(page);
    thumbnails.setCurrent(page);
  },
  onZoomChange: (zoom) => statusbar.setZoom(zoom),
  loadText: (page) => invoke("page_text", { id: current.id, page }),
});
const statusbar = attachStatusbar(viewer);
const search = attachSearch(viewer);
const thumbnails = new Thumbnails(byId("thumbs"), byId("sidebar-body"), (page) => viewer.goToPage(page));
const settingsView = attachSettings({ onSaved: applySettings, onClose: closeSettings });

function show(doc) {
  current = doc;
  showHome(!doc);
  byId("sidebar").hidden = !doc;
  byId("doc-name").textContent = doc ? doc.file_name : "";
  byId("menu-close").disabled = !doc;
  statusbar.setDocument(doc);
  setWindowTitle(doc ? `${doc.title} – PdfEditor` : "PdfEditor").catch(() => {});
  if (doc) {
    viewer.load(doc);
    thumbnails.load(doc);
    const hasOutline = renderOutline(byId("outline"), doc.outline, (page) => viewer.goToPage(page));
    showSidebarTab(hasOutline && doc.pages.length > 1 ? "outline" : "pages");
    statusbar.setPage(0);
  } else {
    viewer.unload();
    thumbnails.clear();
    refreshRecent();
  }
  search.reset();
}

/** Ouvre un fichier ; le document précédent n'est fermé qu'en cas de succès. */
async function open(path) {
  const doc = await openPath(path);
  if (!doc) return;
  const previous = current;
  show(doc);
  if (previous) invoke("close_document", { id: previous.id });
}

function close() {
  if (!current) return;
  invoke("close_document", { id: current.id });
  show(null);
}

async function chooseAndOpen() {
  const path = await pickPdf();
  if (path) await open(path);
}

async function refreshRecent() {
  const files = await invoke("recent_files").catch(() => []);
  renderRecent(files, open);
  return files;
}

function showSidebarTab(tab) {
  const outline = tab === "outline";
  for (const [id, selected] of [["tab-pages", !outline], ["tab-outline", outline]]) {
    byId(id).classList.toggle("on", selected);
    byId(id).classList.toggle("ghost", !selected);
    byId(id).setAttribute("aria-selected", String(selected));
  }
  byId("thumbs").hidden = outline;
  byId("outline").hidden = !outline;
}

/** Réglages appliqués à chaud : thème, zoom d'ouverture, raccourcis. */
function applySettings(saved) {
  settings = saved;
  applyTheme(saved.general.theme);
  viewer.defaultZoom = saved.display.default_zoom;
  shortcuts = keymap(saved.shortcuts);
  for (const kbd of document.querySelectorAll("kbd[data-shortcut]")) {
    kbd.textContent = shortcutLabel(saved.shortcuts[kbd.dataset.shortcut] ?? "");
  }
  for (const button of document.querySelectorAll("[data-shortcut-hint]")) {
    const label = button.getAttribute("aria-label") ?? "";
    const base = label.replace(/ \(.*\)$/, "");
    const shortcut = saved.shortcuts[button.dataset.shortcutHint];
    const text = shortcut ? `${base} (${shortcutLabel(shortcut)})` : base;
    button.setAttribute("aria-label", text);
    button.title = text;
  }
  if (!current) refreshRecent();
}

function openSettings() {
  byId("main-menu").hidden = true;
  document.querySelector(".app").hidden = true;
  byId("settings-view").hidden = false;
  settingsView.open();
}

function closeSettings() {
  byId("settings-view").hidden = true;
  document.querySelector(".app").hidden = false;
}

const actions = {
  open: chooseAndOpen,
  close,
  search: () => search.open(),
  zoom_in: () => viewer.setZoom(zoomIn(viewer.zoom)),
  zoom_out: () => viewer.setZoom(zoomOut(viewer.zoom)),
  zoom_reset: () => viewer.setZoom(1),
  fit_page: () => viewer.fit("page"),
  fit_width: () => viewer.fit("width"),
  settings: openSettings,
};

byId("tab-pages").addEventListener("click", () => showSidebarTab("pages"));
byId("tab-outline").addEventListener("click", () => showSidebarTab("outline"));
byId("home-open").addEventListener("click", chooseAndOpen);
byId("menu-open").addEventListener("click", chooseAndOpen);
byId("menu-close").addEventListener("click", close);
byId("menu-settings").addEventListener("click", openSettings);
byId("recent-clear").addEventListener("click", async () => {
  await invoke("clear_recent_files").catch((e) => toast(e?.message ?? String(e), "error"));
  refreshRecent();
});
attachMenu(byId("menu-button"), byId("main-menu"));
watchDrops(open);

// Ctrl + molette : zoom, comme dans les autres visionneuses.
byId("desk").addEventListener(
  "wheel",
  (event) => {
    if (!event.ctrlKey || !current) return;
    event.preventDefault();
    viewer.setZoom(event.deltaY < 0 ? zoomIn(viewer.zoom) : zoomOut(viewer.zoom));
  },
  { passive: false },
);

document.addEventListener("keydown", (event) => {
  // Page Réglages ouverte : seuls ses propres contrôles réagissent.
  if (!byId("settings-view").hidden) return;
  const shortcut = eventToShortcut(event);
  const action = shortcut && shortcuts.get(shortcut);
  if (!action) return;
  event.preventDefault();
  actions[action]();
});

async function start() {
  invoke("app_info")
    .then((info) => {
      byId("app-version").textContent = t("settings.version", { version: info.version });
    })
    .catch(() => {});
  try {
    const view = await invoke("get_settings");
    settingsView.load(view);
    applySettings(view.settings);
    if (view.warnings.length > 0) toast(view.warnings.join(" "), "error");
  } catch (error) {
    toast(t("error.unexpected", { message: error?.message ?? String(error) }), "error");
  }
  show(null);
  attachUpdates();
  // Fichier passé en argument (« Ouvrir avec PdfEditor »), sinon le dernier
  // document si le réglage le demande.
  const files = await invoke("initial_files").catch(() => []);
  if (files[0]) return open(files[0]);
  if (settings?.general.reopen_last) {
    const recent = await refreshRecent();
    if (recent[0]?.exists) return open(recent[0].path);
  }
}

start();
