// Gestes au doigt sur la table : calculs du pincement, du double
// tapotement et du défilement qui garde un point sous les doigts. Module
// pur, testé par touch-zoom.test.js ; le DOM est dans touch-gestures.js.

import { clampZoom } from "./zoom.js";

/** Au-delà de ce déplacement (px CSS), un appui n'est plus un tapotement. */
export const TAP_SLOP = 10;
/** Deux tapotements à moins de ce délai (ms) et de cette distance (px) : un double. */
export const DOUBLE_TAP_MS = 300;
export const DOUBLE_TAP_DISTANCE = 30;
/** Grossissement d'un double tapotement depuis l'ajustement à la largeur. */
export const DOUBLE_TAP_FACTOR = 2;

export function distance(a, b) {
  return Math.hypot(a.x - b.x, a.y - b.y);
}

export function midpoint(a, b) {
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2 };
}

/**
 * Zoom pendant un pincement : zoom de départ × écartement relatif des
 * doigts, borné comme tout zoom.
 */
export function pinchZoom(startZoom, startDistance, currentDistance) {
  if (!(startDistance > 0) || !(currentDistance > 0)) return clampZoom(startZoom);
  return clampZoom((startZoom * currentDistance) / startDistance);
}

/** Le tapotement `tap` ({x, y, time}) complète-t-il `previous` en double tapotement ? */
export function isDoubleTap(previous, tap) {
  if (!previous) return false;
  return tap.time - previous.time <= DOUBLE_TAP_MS && distance(previous, tap) <= DOUBLE_TAP_DISTANCE;
}

/**
 * Double tapotement : ajusté à la largeur, la page grossit sous le doigt ;
 * sinon elle revient à la largeur de l'écran.
 * Renvoie { zoom, fit } : `fit` vaut "width" pour un retour à la largeur.
 */
export function doubleTapTarget(zoom, fitMode, widthZoom) {
  const atWidth = fitMode === "width" || Math.abs(zoom - widthZoom) < 0.01;
  if (atWidth) return { zoom: clampZoom(widthZoom * DOUBLE_TAP_FACTOR), fit: null };
  return { zoom: widthZoom, fit: "width" };
}

/**
 * Défilement horizontal après un zoom de rapport `ratio` qui garde sous
 * l'abscisse `to` (px, dans la zone qui défile) le point du contenu qui
 * était sous `from`. Le contenu est centré dans un conteneur de largeur
 * `size` placé à `offset` ; avant et après le zoom.
 */
export function scrollLeftForFocus({ scroll, from, to, ratio, before, after }) {
  const fromCenter = scroll + from - before.offset - before.size / 2;
  return Math.max(0, after.offset + after.size / 2 + fromCenter * ratio - to);
}
