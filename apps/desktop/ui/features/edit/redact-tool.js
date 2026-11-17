// Outil Caviarder (E-07) : un glisser qui part d'un mot caviarde le texte
// sélectionné ; ailleurs sur la page, il trace une zone rectangulaire.
// L'élément est créé au relâchement ; le moteur supprime ensuite
// réellement ce qui se trouve dessous.

import { newRedaction } from "./annotation-state.js";
import { startDrawing } from "./draw-tool.js";
import { trackTextSelection } from "./markup-tool.js";

/**
 * Surveille la table : outil actif (`enabled()`), `onRedact(élément)` au
 * relâchement d'un glisser sur une page.
 */
export function attachRedactTool(viewer, enabled, onRedact) {
  viewer.desk.addEventListener("pointerdown", (event) => {
    if (!enabled() || event.button !== 0) return;
    const frame = event.target.closest?.(".page");
    const index = viewer.frames.indexOf(frame);
    const page = viewer.doc?.pages[index];
    if (!page) return;
    event.preventDefault();
    const layer = frame.querySelector(".text-layer");
    const onWord = event.target.closest?.(".text-layer span");
    if (onWord && layer) {
      trackTextSelection(event, layer, page, (rects) => {
        const redaction = newRedaction(index, rects);
        if (redaction) onRedact(redaction);
      });
      return;
    }
    const editLayer = frame.querySelector(".edit-layer");
    if (!editLayer) return;
    startDrawing(event, editLayer, index, page, "redact", {}, (element) => {
      if (element) onRedact(element);
    });
  });
}
