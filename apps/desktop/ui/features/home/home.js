// Accueil (aucun document) et glisser-déposer d'un fichier sur la fenêtre.

import { byId } from "../../lib/dom.js";
import { listen } from "../../lib/ipc.js";

export function showHome(visible) {
  byId("home").hidden = !visible;
}

/**
 * Appelle `onDrop(chemin, point)` pour le premier fichier déposé sur la
 * fenêtre ; `point` : position en pixels CSS, si Tauri la donne.
 */
export function watchDrops(onDrop) {
  const sheet = byId("drop-sheet");
  listen("tauri://drag-enter", () => sheet.classList.add("dragging"));
  listen("tauri://drag-leave", () => sheet.classList.remove("dragging"));
  listen("tauri://drag-drop", (event) => {
    sheet.classList.remove("dragging");
    const paths = event.payload?.paths ?? [];
    const [path] = paths;
    const position = event.payload?.position;
    // Tauri donne des pixels physiques.
    const ratio = window.devicePixelRatio || 1;
    const point = position ? { x: position.x / ratio, y: position.y / ratio } : null;
    if (path) onDrop(path, point, paths);
  });
}
