// Démarrage de l'interface : assemble les modules et gère le document ouvert.
// Un seul document à la fois en v0.1 (onglets : V-06).

import { attachMenu } from "./components/menu.js";
import { showHome, watchDrops } from "./features/home/home.js";
import { openPath, pickPdf } from "./features/open/open.js";
import { attachSearch } from "./features/search/search-bar.js";
import { renderOutline } from "./features/viewer/outline.js";
import { attachStatusbar } from "./features/viewer/statusbar.js";
import { Thumbnails } from "./features/viewer/thumbnails.js";
import { Viewer } from "./features/viewer/viewer.js";
import { zoomIn, zoomOut } from "./features/viewer/zoom.js";
import { byId } from "./lib/dom.js";
import { translate } from "./lib/i18n.js";
import { invoke, setWindowTitle } from "./lib/ipc.js";

translate(document);

let current = null;

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

function showSidebarTab(tab) {
  const outline = tab === "outline";
  for (const [id, selected] of [["tab-pages", !outline], ["tab-outline", outline]]) {
    byId(id).classList.toggle("on", selected);
    byId(id).classList.toggle("ghost", !selected);
  }
  byId("tab-pages").setAttribute("aria-selected", String(!outline));
  byId("tab-outline").setAttribute("aria-selected", String(outline));
  byId("thumbs").hidden = outline;
  byId("outline").hidden = !outline;
}

byId("tab-pages").addEventListener("click", () => showSidebarTab("pages"));
byId("tab-outline").addEventListener("click", () => showSidebarTab("outline"));
byId("home-open").addEventListener("click", chooseAndOpen);
byId("menu-open").addEventListener("click", chooseAndOpen);
byId("menu-close").addEventListener("click", close);
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

const shortcuts = {
  f: () => search.open(),
  o: chooseAndOpen,
  w: close,
  "+": () => viewer.setZoom(zoomIn(viewer.zoom)),
  "=": () => viewer.setZoom(zoomIn(viewer.zoom)),
  "-": () => viewer.setZoom(zoomOut(viewer.zoom)),
  0: () => viewer.setZoom(1),
  1: () => viewer.fit("page"),
  2: () => viewer.fit("width"),
};

document.addEventListener("keydown", (event) => {
  if (!(event.ctrlKey || event.metaKey) || event.altKey) return;
  const action = shortcuts[event.key.toLowerCase()];
  if (!action) return;
  event.preventDefault();
  action();
});

show(null);
// Fichier passé en argument (« Ouvrir avec PdfEditor »).
invoke("initial_files")
  .then((files) => files[0] && open(files[0]))
  .catch(() => {});
