// Sélection de plusieurs éléments (E-03), sans DOM : ordre de tabulation,
// alignement, répartition, magnétisme. Module pur, testé par
// selection-state.test.js. Coordonnées : points de la page affichée,
// origine en haut à gauche ; `bounds` : place donnée par le moteur.

import { moved } from "./edit-state.js";

/** Éléments dans l'ordre de lecture : page, puis haut, puis gauche. */
export function readingOrder(items) {
  return [...items].sort(
    (a, b) => a.element.page - b.element.page || a.bounds.y - b.bounds.y || a.bounds.x - b.bounds.x,
  );
}

/** Élément suivant (ou précédent) pour Tab / Maj+Tab, en boucle. */
export function nextInOrder(items, currentId, backward = false) {
  const order = readingOrder(items);
  if (order.length === 0) return null;
  const index = order.findIndex((item) => item.id === currentId);
  if (index < 0) return (backward ? order[order.length - 1] : order[0]).id;
  const step = backward ? -1 : 1;
  return order[(index + step + order.length) % order.length].id;
}

/** Cadre commun à plusieurs boîtes. */
export function union(boxes) {
  const left = Math.min(...boxes.map((b) => b.x));
  const top = Math.min(...boxes.map((b) => b.y));
  const right = Math.max(...boxes.map((b) => b.x + b.width));
  const bottom = Math.max(...boxes.map((b) => b.y + b.height));
  return { x: left, y: top, width: right - left, height: bottom - top };
}

/** Tous les éléments sur la même page (alignement possible). */
export const samePage = (items) => items.every((item) => item.element.page === items[0]?.element.page);

/**
 * Alignement des éléments sur leur cadre commun : "left", "center",
 * "right" (horizontal), "top", "middle", "bottom" (vertical).
 * Renvoie [{ id, element }] pour les éléments qui bougent.
 */
export function aligned(items, mode, page) {
  const all = union(items.map((i) => i.bounds));
  const target = {
    left: (b) => [all.x - b.x, 0],
    center: (b) => [all.x + all.width / 2 - (b.x + b.width / 2), 0],
    right: (b) => [all.x + all.width - (b.x + b.width), 0],
    top: (b) => [0, all.y - b.y],
    middle: (b) => [0, all.y + all.height / 2 - (b.y + b.height / 2)],
    bottom: (b) => [0, all.y + all.height - (b.y + b.height)],
  }[mode];
  return items
    .map((item) => {
      const [dx, dy] = target(item.bounds);
      return { id: item.id, dx, dy, item };
    })
    .filter(({ dx, dy }) => Math.abs(dx) > 1e-6 || Math.abs(dy) > 1e-6)
    .map(({ id, dx, dy, item }) => ({ id, element: moved(item.element, dx, dy, page) }));
}

/**
 * Répartition régulière (3 éléments ou plus) : écarts égaux entre les
 * éléments, les deux extrêmes restent en place. `axis` : "x" ou "y".
 */
export function distributed(items, axis, page) {
  if (items.length < 3) return [];
  const [start, size] = axis === "x" ? ["x", "width"] : ["y", "height"];
  const sorted = [...items].sort((a, b) => a.bounds[start] - b.bounds[start]);
  const first = sorted[0].bounds;
  const last = sorted[sorted.length - 1].bounds;
  const span = last[start] + last[size] - first[start];
  const filled = sorted.reduce((sum, i) => sum + i.bounds[size], 0);
  const gap = (span - filled) / (sorted.length - 1);
  let cursor = first[start];
  const result = [];
  for (const item of sorted) {
    const delta = cursor - item.bounds[start];
    if (Math.abs(delta) > 1e-6) {
      const [dx, dy] = axis === "x" ? [delta, 0] : [0, delta];
      result.push({ id: item.id, element: moved(item.element, dx, dy, page) });
    }
    cursor += item.bounds[size] + gap;
  }
  return result;
}

/** Repères d'un cadre sur un axe : début, milieu, fin. */
function marks(box, axis) {
  const [start, size] = axis === "x" ? [box.x, box.width] : [box.y, box.height];
  return [start, start + size / 2, start + size];
}

/**
 * Magnétisme d'un déplacement : le cadre `box` déplacé de (dx, dy) se cale
 * sur le repère le plus proche (bords et centres des `others`, de la page)
 * à moins de `threshold` points ; sinon, sur la grille (`grid` points, 0 :
 * aucune). Renvoie le déplacement corrigé et les repères à afficher.
 */
export function snapped(box, dx, dy, others, page, threshold, grid = 0) {
  const pageBox = { x: 0, y: 0, width: page.width, height: page.height };
  const result = { dx, dy, guides: [] };
  for (const axis of ["x", "y"]) {
    const delta = axis === "x" ? dx : dy;
    const moving = marks(axis === "x" ? { ...box, x: box.x + delta } : { ...box, y: box.y + delta }, axis);
    let best = null;
    for (const target of [pageBox, ...others]) {
      for (const at of marks(target, axis)) {
        for (const mine of moving) {
          const offset = at - mine;
          if (Math.abs(offset) <= threshold && (!best || Math.abs(offset) < Math.abs(best.offset))) {
            best = { offset, at };
          }
        }
      }
    }
    let corrected = delta;
    if (best) {
      corrected = delta + best.offset;
      result.guides.push({ axis, at: best.at });
    } else if (grid > 0) {
      const start = moving[0];
      corrected = delta + (Math.round(start / grid) * grid - start);
    }
    if (axis === "x") result.dx = corrected;
    else result.dy = corrected;
  }
  return result;
}
