// Calculs de l'édition, sans DOM : conversions d'unités, création,
// déplacement, redimensionnement et style d'une zone de texte ; ce qui est
// propre aux images est dans image-state.js. Module pur, testé par
// edit-state.test.js.
//
// Coordonnées : points de la page affichée, origine en haut à gauche
// (celles qu'échangent l'interface et pde-app, voir edit_view.rs).

import { withSize } from "./image-state.js";

export const PT_PER_MM = 72 / 25.4;
/** Largeur d'une nouvelle zone de texte (points). */
export const NEW_TEXT_WIDTH = 200;
/** Largeur minimale à la souris (pde-core refuse en dessous de 8 pt). */
export const MIN_TEXT_WIDTH = 24;
/** Marge gardée entre une zone et le bord de la page. */
const PAGE_MARGIN = 4;

export const mmFromPt = (pt) => Math.round((pt / PT_PER_MM) * 10) / 10;
export const ptFromMm = (mm) => mm * PT_PER_MM;

/** Point de la page (points) sous un pixel de son cadre affiché. */
export function pagePoint(offsetX, offsetY, frameWidth, frameHeight, page) {
  return {
    x: (offsetX / frameWidth) * page.width,
    y: (offsetY / frameHeight) * page.height,
  };
}

/** Pixels CSS → points de la page, pour un déplacement à la souris. */
export function pointsFromPixels(dx, dy, frameWidth, page) {
  const ratio = page.width / frameWidth;
  return { dx: dx * ratio, dy: dy * ratio };
}

function clamp(value, min, max) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

/** Nouvelle zone de texte à l'endroit cliqué, gardée dans la page. */
export function newTextBox(pageIndex, point, page, style) {
  const width = clamp(NEW_TEXT_WIDTH, MIN_TEXT_WIDTH, page.width - 2 * PAGE_MARGIN);
  return {
    kind: "text",
    page: pageIndex,
    top_left: {
      x: clamp(point.x, PAGE_MARGIN, page.width - width - PAGE_MARGIN),
      y: clamp(point.y, PAGE_MARGIN, page.height - style.size - PAGE_MARGIN),
    },
    width,
    text: "",
    style: { ...style },
  };
}

/** Hauteur gardée dans la page : le cadre d'une image, une ligne de texte. */
function keptHeight(element) {
  return element.kind === "image" ? element.height : element.style.size;
}

/** Élément déplacé de (dx, dy) points, sans sortir de la page. */
export function moved(element, dx, dy, page) {
  return {
    ...element,
    top_left: {
      x: clamp(element.top_left.x + dx, 0, page.width - element.width),
      y: clamp(element.top_left.y + dy, 0, page.height - keptHeight(element)),
    },
  };
}

/** Largeur changée en tirant le bord gauche ou droit de `dx` points. */
export function resized(element, edge, dx, page) {
  const { x } = element.top_left;
  const right = x + element.width;
  if (edge === "left") {
    const left = clamp(x + dx, 0, right - MIN_TEXT_WIDTH);
    return { ...element, top_left: { ...element.top_left, x: left }, width: right - left };
  }
  const newRight = clamp(right + dx, x + MIN_TEXT_WIDTH, page.width);
  return { ...element, width: newRight - x };
}

/** Flèches du clavier : 1 point, 10 avec Maj. `null` pour une autre touche. */
export function nudged(element, key, shift, page) {
  const step = shift ? 10 : 1;
  const moves = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] };
  const move = moves[key];
  return move ? moved(element, move[0], move[1], page) : null;
}

export function withStyle(element, changes) {
  return { ...element, style: { ...element.style, ...changes } };
}

export function withText(element, text) {
  return { ...element, text };
}

/** Police d'affichage la plus proche, pour la saisie directe sur la page. */
export function cssFontFamily(font) {
  return {
    sans: 'Helvetica, Arial, "Liberation Sans", sans-serif',
    serif: '"Times New Roman", Times, "Liberation Serif", serif',
    mono: '"Courier New", Courier, "Liberation Mono", monospace',
  }[font];
}

export function hexFromColor({ r, g, b }) {
  return `#${[r, g, b].map((v) => v.toString(16).padStart(2, "0")).join("")}`;
}

export function colorFromHex(hex) {
  const match = /^#([0-9a-f]{2})([0-9a-f]{2})([0-9a-f]{2})$/i.exec(hex);
  if (!match) return null;
  return { r: parseInt(match[1], 16), g: parseInt(match[2], 16), b: parseInt(match[3], 16) };
}

/** Couleurs proposées en un clic (couleurs du document, pas de l'interface). */
export const SWATCHES = [
  { name: "ink", hex: "#1c2430" },
  { name: "blue", hex: "#2747b8" },
  { name: "seal", hex: "#8a1c2b" },
  { name: "green", hex: "#1e7344" },
  { name: "white", hex: "#ffffff" },
];

/** Corps saisi au clavier : nombre borné comme dans pde-core, sinon null. */
export function parseSize(value) {
  const size = Number.parseFloat(String(value).replace(",", "."));
  if (!Number.isFinite(size)) return null;
  return Math.min(Math.max(Math.round(size * 2) / 2, 4), 200);
}

/** Copie décalée vers le bas à droite, gardée dans la page. */
export function duplicated(element, page) {
  return moved(element, 12, 12, page);
}

/** Position et largeur affichées en mm (panneau Propriétés). */
export function geometryInMm(element, bounds) {
  return {
    x: mmFromPt(element.top_left.x),
    y: mmFromPt(element.top_left.y),
    width: mmFromPt(element.width),
    height: mmFromPt(bounds.height),
  };
}

/** Position ou largeur saisie en mm (virgule acceptée) ; null si invalide. */
export function withGeometryMm(element, field, value, page) {
  const mm = Number.parseFloat(String(value).replace(",", "."));
  if (!Number.isFinite(mm)) return null;
  const pt = ptFromMm(mm);
  if (element.kind === "image" && (field === "width" || field === "height")) return withSize(element, field, pt, page);
  if (field === "width") return resized(element, "right", pt - element.width, page);
  const dx = field === "x" ? pt - element.top_left.x : 0;
  const dy = field === "y" ? pt - element.top_left.y : 0;
  return moved(element, dx, dy, page);
}
