// Tracé à la souris avec les outils Forme, Dessin et Caviarder (zone) :
// aperçu SVG (en points de la page) pendant le glisser, élément créé au
// relâchement. Échap annule.

import { annotationHex } from "./annotation-controls.js";
import { constrained, newInk, newRedactArea, newShape, withPoint } from "./annotation-state.js";
import { pagePoint } from "./edit-state.js";

const SVG = "http://www.w3.org/2000/svg";

function svg(tag, attributes = {}) {
  const node = document.createElementNS(SVG, tag);
  for (const [name, value] of Object.entries(attributes)) node.setAttribute(name, String(value));
  return node;
}

/** Pointe de flèche (même calcul que pde-core, pour l'aperçu seulement). */
function arrowHead(start, end, width) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  const length = Math.max(Math.hypot(dx, dy), Number.EPSILON);
  const [ux, uy] = [dx / length, dy / length];
  const size = Math.min(Math.max(4 * width, 8), length);
  const half = size * 0.45;
  const base = { x: end.x - ux * size, y: end.y - uy * size };
  return [end, { x: base.x - uy * half, y: base.y + ux * half }, { x: base.x + uy * half, y: base.y - ux * half }];
}

/** Élément d'aperçu pour l'outil et les points en cours. */
function preview(tool, style, start, end, points) {
  if (tool === "redact") {
    const [x, y] = [Math.min(start.x, end.x), Math.min(start.y, end.y)];
    return svg("rect", { class: "redact-draft", x, y, width: Math.abs(end.x - start.x), height: Math.abs(end.y - start.y) });
  }
  const color = annotationHex(style.color);
  const stroke = { stroke: color, "stroke-width": style.width, fill: "none", "stroke-linecap": "round", "stroke-linejoin": "round" };
  if (tool === "ink") return svg("polyline", { ...stroke, points: points.map((p) => `${p.x},${p.y}`).join(" ") });
  const fill = style.filled ? color : "none";
  const x = Math.min(start.x, end.x);
  const y = Math.min(start.y, end.y);
  const w = Math.abs(end.x - start.x);
  const h = Math.abs(end.y - start.y);
  if (style.shape === "rectangle") return svg("rect", { ...stroke, x, y, width: w, height: h, fill, "fill-opacity": 0.25 });
  if (style.shape === "ellipse") return svg("ellipse", { ...stroke, cx: x + w / 2, cy: y + h / 2, rx: w / 2, ry: h / 2, fill, "fill-opacity": 0.25 });
  const group = svg("g");
  group.append(svg("line", { ...stroke, x1: start.x, y1: start.y, x2: end.x, y2: end.y }));
  if (style.shape === "arrow") {
    group.append(svg("polygon", { fill: color, points: arrowHead(start, end, style.width).map((p) => `${p.x},${p.y}`).join(" ") }));
  }
  return group;
}

/**
 * Commence un tracé sur la couche d'une page ; `onDone(élément | null)` au
 * relâchement (null : annulé ou trop court).
 */
export function startDrawing(event, layer, pageIndex, page, tool, style, onDone) {
  const rect = layer.getBoundingClientRect();
  const toPage = (e) => pagePoint(e.clientX - rect.left, e.clientY - rect.top, rect.width, rect.height, page);
  const start = toPage(event);
  let end = start;
  let points = [start];
  const draft = svg("svg", { class: "draft", viewBox: `0 0 ${page.width} ${page.height}`, preserveAspectRatio: "none", "aria-hidden": "true" });
  layer.append(draft);

  const draw = () => draft.replaceChildren(preview(tool, style, start, end, points));
  const finish = (element) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key, true);
    draft.remove();
    onDone(element);
  };
  const move = (e) => {
    const p = toPage(e);
    if (tool === "ink") points = withPoint(points, p, page);
    else end = e.shiftKey ? constrained(style.shape, start, p) : p;
    draw();
  };
  const up = () => {
    if (tool === "redact") return finish(newRedactArea(pageIndex, start, end, page));
    finish(tool === "ink" ? newInk(pageIndex, points, style) : newShape(pageIndex, start, end, style, page));
  };
  const key = (e) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    finish(null);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key, true);
  draw();
}
