// Démarrage de l'interface : assemble les modules, applique les réglages et
// gère le document ouvert. Un seul document à la fois en v0.1 (onglets : V-06).

import { attachMenu } from "./components/menu.js";
import { toast } from "./components/toast.js";
import { EditMode } from "./features/edit/edit-mode.js";
import { isImagePath } from "./features/edit/image-state.js";
import { confirmDiscard, saveDocument, saveDocumentAs } from "./features/edit/saving.js";
import { showHome, watchDrops } from "./features/home/home.js";
import { offerRecoveries } from "./features/recovery/recovery.js";
import { renderRecent } from "./features/home/recent.js";
import { openPath, pickPdf } from "./features/open/open.js";
import { PagesMode } from "./features/pages/pages-mode.js";
import { FormLayer } from "./features/form/form-layer.js";
import { FormDesign } from "./features/form-design/form-design.js";
import { attachProperties } from "./features/form/properties-dialog.js";
import { attachSearch } from "./features/search/search-bar.js";
import { SignMode } from "./features/sign/sign-mode.js";
import { attachSettings } from "./features/settings/settings-view.js";
import { attachUpdates } from "./features/updates/updates.js";
import { renderOutline } from "./features/viewer/outline.js";
import { attachStatusbar } from "./features/viewer/statusbar.js";
import { Thumbnails } from "./features/viewer/thumbnails.js";
import { Viewer } from "./features/viewer/viewer.js";
import { attachDeskInput } from "./features/viewer/desk-input.js";
import { attachKeyboard } from "./features/palette/keyboard.js";
import { Palette } from "./features/palette/palette.js";
import { showSidebarTab } from "./features/viewer/sidebar.js";
import { AiMode } from "./features/ai/ai-mode.js";
import { attachAiSettings } from "./features/ai/ai-settings.js";
import { attachRecipesSettings } from "./features/ai/recipes-settings.js";
import { zoomIn, zoomOut } from "./features/viewer/zoom.js";
import { byId } from "./lib/dom.js";
import { t, translate } from "./lib/i18n.js";
import { invoke, listen, setWindowTitle } from "./lib/ipc.js";
import { shortcutLabel, keymap } from "./lib/shortcuts.js";
import { applyTheme } from "./lib/theme.js";

translate(document);

let current = null;
let settings = null;
let shortcuts = new Map();
let mode = "read";
/** Heure de la dernière copie de secours du document affiché (F-02). */
let backedUpAt = null;

const viewer = new Viewer(byId("desk"), {
  onPageChange: (page) => {
    statusbar.setPage(page);
    thumbnails.setCurrent(page);
  },
  onZoomChange: (zoom) => statusbar.setZoom(zoom),
  loadText: (page) => invoke("page_text", { id: current.id, page }),
  onPageShown: (frame, page) => {
    edit.layer.pageShown(frame, page);
    form.pageShown(frame, page);
    formDesign.pageShown(frame, page);
    sign.pageShown(frame, page);
  },
});
const statusbar = attachStatusbar(viewer);
const search = attachSearch(viewer);
const thumbnails = new Thumbnails(byId("thumbs"), byId("sidebar-body"), (page) => viewer.goToPage(page));
const settingsView = attachSettings({
  onSaved: applySettings,
  onClose: closeSettings,
  onFill: (draft) => aiSettings.fill(draft),
});
const aiSettings = attachAiSettings(settingsView);
// Recettes de l'assistant (IA-16) : rubrique des Réglages, choix dans le mode IA.
const recipes = attachRecipesSettings((list) => ai.setRecipes(list));
// IA (v0.5) : panneau Assistant IA en mode IA ; actions acceptées de
// l'assistant appliquées comme une opération d'édition.
const ai = new AiMode(viewer, {
  openSettings: () => openSettings("ai"),
  applied: (result) => edit.session.apply(result.edit, result.document),
  howTo: (query) => palette.open("help", query),
});
const edit = new EditMode(viewer, (view, changed) => {
  if (changed.length > 0) thumbnails.setRevisions(view.revisions, changed);
  showSaveState(view.dirty);
});
// Double-clic (ou Entrée) sur une page du mode Pages : ouverte en mode Lire.
const pages = new PagesMode(edit.session, (page) => {
  setMode("read");
  viewer.goToPage(page);
});
edit.session.onLayout(relayout);
// Formulaire en mode Lire, propriétés du document dans le menu.
const form = new FormLayer(edit.session, viewer);
// Conception de formulaires (E-12) : mode Formulaire ; « Tester » passe en mode Lire.
const formDesign = new FormDesign(edit.session, viewer, {
  test: () => setMode("read"),
  fieldsChanged: (fields) => form.setFields(fields),
});
attachProperties(
  () => current,
  edit.session,
  (title) => setWindowTitle(`${title || current.file_name} – PdfEditor`).catch(() => {}),
);

// Signature (S-01 à S-07) : le fichier signé est rouvert ; un document
// verrouillé par sa signature n'est plus modifiable.
const sign = new SignMode(viewer, {
  session: edit.session,
  save: async () => {
    const saved = await saveDocument(edit.session);
    renamed(saved);
    return saved;
  },
  reopen: open,
  onState: applyLock,
  onSeal: () => setMode("sign"),
});

function applyLock(info) {
  if (info.lock && (mode === "edit" || mode === "pages" || mode === "form")) setMode("read");
  form.flattenAllowed = !info.lock;
  form.setActive(mode === "read" && info.lock !== "all");
}

/**
 * Pages changées (mode Pages, ou son annulation) : tout ce qui montre les
 * pages est refait ; le nom du fichier reste celui affiché.
 */
function relayout(doc) {
  if (!current) return;
  Object.assign(current, { pages: doc.pages, outline: doc.outline, title: doc.title });
  const page = Math.min(viewer.page, doc.pages.length - 1);
  viewer.reload(current, page);
  thumbnails.load(current);
  thumbnails.setCurrent(page);
  renderOutline(byId("outline"), current.outline, (p) => viewer.goToPage(p));
  statusbar.setDocument(current);
  statusbar.setPage(page);
  pages.load(current);
  form.load(current);
  formDesign.load(current);
  search.reset();
}

/** Pastille et barre d'état « modifié » ; Enregistrer actif s'il y a à faire. */
function showSaveState(dirty) {
  if (!dirty) backedUpAt = null;
  byId("doc-dirty").hidden = !dirty;
  byId("save-button").disabled = !dirty;
  byId("menu-save").disabled = !dirty;
  const time = backedUpAt?.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  const state = !dirty ? "status.saved" : time ? "status.unsavedBackup" : "status.unsaved";
  byId("save-state").textContent = current ? t(state, { time }) : "";
}

/** Lire, Éditer, Pages, Signer ou IA. */
function setMode(next) {
  const previous = mode;
  mode = current ? next : "read";
  for (const name of ["read", "edit", "form", "pages", "sign", "ai"]) byId(`mode-${name}`).toggleAttribute("aria-current", false);
  byId(`mode-${mode}`).setAttribute("aria-current", "page");
  byId("save-button").hidden = mode === "read" || mode === "sign" || mode === "ai";
  if (mode !== "edit") edit.layer.commitText();
  edit.setActive(mode === "edit");
  formDesign.setActive(mode === "form");
  pages.setActive(mode === "pages", viewer.page);
  sign.setActive(mode === "sign");
  ai.setActive(mode === "ai");
  form.setActive(mode === "read" && sign.info.lock !== "all");
  if (mode === "edit" || mode === "pages" || mode === "form") sign.warnIfSigned(current);
  // En quittant la grille, la page choisie est affichée.
  if (previous === "pages" && mode !== "pages" && current) viewer.goToPage(pages.firstSelected);
}

/** Affiche `doc` (ou l'accueil) ; la promesse suit le chargement de son édition. */
function show(doc) {
  current = doc;
  backedUpAt = null;
  let editLoading = Promise.resolve();
  showHome(!doc);
  byId("sidebar").hidden = !doc;
  byId("doc-name").textContent = doc ? doc.file_name : "";
  byId("menu-close").disabled = !doc;
  byId("menu-save-as").disabled = !doc;
  byId("mode-edit").disabled = !doc;
  byId("mode-form").disabled = !doc;
  byId("mode-pages").disabled = !doc;
  byId("mode-sign").disabled = !doc;
  byId("mode-ai").disabled = !doc;
  ai.load(doc);
  byId("menu-properties").disabled = !doc;
  statusbar.setDocument(doc);
  setWindowTitle(doc ? `${doc.title} – PdfEditor` : "PdfEditor").catch(() => {});
  if (doc) {
    viewer.load(doc);
    thumbnails.load(doc);
    const hasOutline = renderOutline(byId("outline"), doc.outline, (page) => viewer.goToPage(page));
    showSidebarTab(hasOutline && doc.pages.length > 1 ? "outline" : "pages");
    statusbar.setPage(0);
    editLoading = edit
      .load(doc)
      .then(() => Promise.all([pages.load(doc), form.load(doc), formDesign.load(doc), sign.load(doc)]))
      .catch((e) => toast(t("error.unexpected", { message: e?.message ?? String(e) }), "error"));
  } else {
    edit.load(null);
    pages.load(null);
    form.load(null);
    formDesign.load(null);
    sign.load(null);
    viewer.unload();
    thumbnails.clear();
    refreshRecent();
  }
  search.reset();
  setMode(mode);
  showSaveState(false);
  return editLoading;
}

/** Avant de quitter le document : enregistrer, abandonner ou rester. */
async function canLeave() {
  if (!current) return true;
  edit.layer.commitText();
  return confirmDiscard(edit.session, () => saveDocument(edit.session));
}

function renamed(saved) {
  if (!saved || !current) return;
  current.file_name = saved.file_name;
  byId("doc-name").textContent = saved.file_name;
}

async function save() {
  if (!current) return;
  edit.layer.commitText();
  if (edit.session.dirty) renamed(await saveDocument(edit.session));
}

async function saveAs() {
  if (!current) return;
  edit.layer.commitText();
  renamed(await saveDocumentAs(edit.session));
}

/**
 * Ouvre un fichier ; le document précédent n'est fermé qu'en cas de succès.
 * Renvoie le document, édition chargée, ou null.
 */
async function open(path) {
  if (!(await canLeave())) return null;
  const doc = await openPath(path);
  if (!doc) return null;
  const previous = current;
  const loading = show(doc);
  if (previous) invoke("close_document", { id: previous.id });
  await loading;
  return doc;
}

async function close() {
  if (!current || !(await canLeave())) return;
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

/** Réglages appliqués à chaud : thème, zoom d'ouverture, raccourcis. */
function applySettings(saved) {
  settings = saved;
  applyTheme(saved.general.theme);
  viewer.defaultZoom = saved.display.default_zoom;
  shortcuts = keymap(saved.shortcuts);
  edit.applySettings(saved.edit);
  sign.applySettings(saved.signature);
  ai.setLanguage(saved.ai.translation.language);
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

function openSettings(section = "general") {
  byId("main-menu").hidden = true;
  document.querySelector(".app").hidden = true;
  byId("settings-view").hidden = false;
  settingsView.open(section);
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
  undo: () => edit.undo(),
  redo: () => edit.redo(),
  save,
  save_as: saveAs,
  palette: () => palette.open("run"),
  how_to: () => palette.open("help"),
};

// Palette de commandes et « Comment faire ? » (V-08), tirées du catalogue des actions.
const palette = new Palette(
  { hasDocument: () => Boolean(current), mode: () => mode, setMode, shortcuts: actions },
  () => settings?.shortcuts ?? {},
);

byId("home-open").addEventListener("click", chooseAndOpen);
byId("menu-open").addEventListener("click", chooseAndOpen);
byId("menu-close").addEventListener("click", close);
byId("menu-save").addEventListener("click", save);
byId("menu-save-as").addEventListener("click", saveAs);
byId("save-button").addEventListener("click", save);
byId("edit-undo").addEventListener("click", () => edit.undo());
byId("edit-redo").addEventListener("click", () => edit.redo());
byId("mode-read").addEventListener("click", () => setMode("read"));
byId("mode-edit").addEventListener("click", () => setMode("edit"));
byId("mode-form").addEventListener("click", () => setMode("form"));
byId("mode-pages").addEventListener("click", () => setMode("pages"));
byId("mode-sign").addEventListener("click", () => setMode("sign"));
byId("mode-ai").addEventListener("click", () => setMode("ai"));
byId("menu-settings").addEventListener("click", () => openSettings());
byId("menu-palette").addEventListener("click", () => palette.open("run"));
byId("menu-how-to").addEventListener("click", () => palette.open("help"));
byId("recent-clear").addEventListener("click", async () => {
  await invoke("clear_recent_files").catch((e) => toast(e?.message ?? String(e), "error"));
  refreshRecent();
});
attachMenu(byId("menu-button"), byId("main-menu"));
/**
 * Fichier déposé : une image s'ajoute au document ouvert ; un PDF s'ouvre,
 * sauf en mode Pages où ses pages sont insérées là où il tombe.
 */
function dropped(path, point, paths = [path]) {
  if (!isImagePath(path)) {
    if (current && mode === "pages") return pages.insertFiles(paths.filter((p) => !isImagePath(p)), point);
    return open(path);
  }
  if (!current) return toast(t("edit.openFirst"), "error");
  if (mode !== "edit") setMode("edit");
  return edit.dropImage(path, point);
}

watchDrops(dropped);

attachDeskInput(viewer, () => Boolean(current), (file) => {
  // Document verrouillé par sa signature : rien ne s'y ajoute.
  if (byId("mode-edit").disabled) return;
  if (mode !== "edit") setMode("edit");
  edit.pasteImage(file);
});

attachKeyboard({ pages, edit, keymap: () => shortcuts, actions });

async function start() {
  palette.load();
  recipes.refresh();
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
  // Fermeture de la fenêtre avec des modifications : Rust attend la réponse.
  listen("close-requested", async () => {
    if (await canLeave()) invoke("quit_app");
  }).catch(() => {});
  // Copie de secours écrite par Rust : heure dans la barre d'état.
  listen("autosaved", (event) => {
    if (!current || !event.payload?.documents?.includes(current.id)) return;
    backedUpAt = new Date();
    showSaveState(edit.session.dirty);
  }).catch(() => {});
  // Lancement précédent interrompu : ses modifications d'abord.
  const recovered = await offerRecoveries({
    open,
    restored: (view) => {
      setMode("edit");
      // Rust a écrit la copie de secours de ce lancement en reprenant.
      backedUpAt = new Date();
      return edit.session.apply(view);
    },
  });
  if (recovered) return;
  // Fichier passé en argument (« Ouvrir avec PdfEditor » ; sur Android,
  // reçu d'une autre application), sinon le dernier document si le réglage
  // le demande.
  const files = [
    ...((await invoke("initial_files").catch(() => null)) ?? []),
    ...((await invoke("shared_files").catch(() => null)) ?? []),
  ];
  if (files[0]) return open(files[0]);
  if (settings?.general.reopen_last) {
    const recent = await refreshRecent();
    if (recent[0]?.exists) return open(recent[0].path);
  }
}

// Android : un document reçu (« Ouvrir avec », « Partager ») pendant que
// l'application tournait s'ouvre quand elle revient au premier plan.
document.addEventListener("visibilitychange", async () => {
  if (document.hidden) return;
  const files = (await invoke("shared_files").catch(() => null)) ?? [];
  if (files[0]) open(files[0]);
});

start();
