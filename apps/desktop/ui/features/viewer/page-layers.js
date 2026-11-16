// Couches posées sur l'image d'une page : texte sélectionnable (transparent,
// aligné sur le texte du PDF) et surlignage des résultats de recherche.
// Placement en pourcentages : un changement de zoom ne les reconstruit pas.

import { el } from "../../lib/dom.js";
import { percentBox, stretch } from "./text-geometry.js";

/** Police de mesure : n'importe laquelle, l'étirement corrige la largeur. */
const MEASURE_FONT = "sans-serif";
let measureContext = null;

function measure(text, size) {
  measureContext ??= document.createElement("canvas").getContext("2d");
  measureContext.font = `${size}px ${MEASURE_FONT}`;
  return measureContext.measureText(text).width;
}

function place(node, box, page) {
  Object.assign(node.style, percentBox(box, page));
}

/** Couche de texte d'une page (`view` : PageTextView de pde-app). */
export function textLayer(view, page) {
  const layer = el("div", { class: "text-layer" });
  let previous = null;
  for (const run of view.runs) {
    // Changement de ligne : <br> pour que le texte copié garde ses lignes
    // (sans effet sur l'affichage, les segments sont positionnés).
    if (previous && Math.abs(run.y - previous.y) > previous.height / 2) layer.append(el("br"));
    previous = run;
    const span = el("span", {}, run.text);
    place(span, run, page);
    // Corps = hauteur de la boîte, en points × échelle de la page (--scale).
    span.style.fontSize = `calc(var(--scale) * ${run.height}px)`;
    span.style.fontFamily = MEASURE_FONT;
    span.style.transform = `scaleX(${stretch(run, measure(run.text, run.height))})`;
    layer.append(span);
  }
  return layer;
}

/** Surlignages d'une page ; `current` : indice de l'occurrence active ou -1. */
export function highlightLayer(hits, page, current) {
  const layer = el("div", { class: "highlights", "aria-hidden": "true" });
  for (const { hit, index } of hits) {
    for (const rect of hit.rects) {
      const mark = el("div", { class: index === current ? "highlight current" : "highlight" });
      place(mark, rect, page);
      layer.append(mark);
    }
  }
  return layer;
}
