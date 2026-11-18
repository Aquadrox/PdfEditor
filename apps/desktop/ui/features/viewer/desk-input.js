// Gestes de la table de lecture : Ctrl + molette pour zoomer, Ctrl+V d'une
// image pour l'ajouter au document.

import { byId } from "../../lib/dom.js";
import { zoomIn, zoomOut } from "./zoom.js";

/** Champ de saisie : Ctrl+Z / Ctrl+Y, Ctrl+V y gardent leur sens habituel. */
export function isEditable(target) {
  return target instanceof HTMLElement && (target.isContentEditable || /^(INPUT|TEXTAREA|SELECT)$/.test(target.tagName));
}

/**
 * @param {import("./viewer.js").Viewer} viewer
 * @param {() => boolean} hasDocument un document est affiché
 * @param {(file: File) => void} onImage image collée
 */
export function attachDeskInput(viewer, hasDocument, onImage) {
  // Ctrl+V avec une image dans le presse-papiers : ajoutée au document.
  document.addEventListener("paste", (event) => {
    if (!hasDocument() || isEditable(event.target) || !byId("settings-view").hidden) return;
    const file = [...(event.clipboardData?.files ?? [])].find((f) => f.type.startsWith("image/"));
    if (!file) return;
    event.preventDefault();
    onImage(file);
  });

  // Ctrl + molette : zoom, comme dans les autres visionneuses.
  byId("desk").addEventListener(
    "wheel",
    (event) => {
      if (!event.ctrlKey || !hasDocument()) return;
      event.preventDefault();
      viewer.setZoom(event.deltaY < 0 ? zoomIn(viewer.zoom) : zoomOut(viewer.zoom));
    },
    { passive: false },
  );
}
