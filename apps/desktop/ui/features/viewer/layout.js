// Disposition des pages sur la table : positions, pages visibles, page
// courante, zoom de rendu. Module pur, testé par layout.test.js.

import { CSS_PX_PER_POINT } from "./zoom.js";

/** Espace entre deux pages et au-dessus de la première (px CSS). */
export const PAGE_GAP = 16;
export const DESK_PADDING = 28;
/** Limites du moteur (pde-core::render) : zoom de rendu et surface d'image. */
export const MAX_RENDER_SCALE = 16;
export const MAX_RENDER_PIXELS = 40_000_000;

/**
 * Position de chaque page pour un zoom donné.
 * @param {{width: number, height: number}[]} pages tailles en points
 */
export function computeLayout(pages, zoom) {
  let top = DESK_PADDING;
  let maxWidth = 0;
  const boxes = pages.map((page) => {
    const width = page.width * CSS_PX_PER_POINT * zoom;
    const height = page.height * CSS_PX_PER_POINT * zoom;
    const box = { top, width, height };
    top += height + PAGE_GAP;
    maxWidth = Math.max(maxWidth, width);
    return box;
  });
  return { boxes, totalHeight: top - PAGE_GAP + DESK_PADDING, maxWidth };
}

/** Première page dont le bas dépasse `y` (recherche dichotomique). */
function firstEndingAfter(boxes, y) {
  let low = 0;
  let high = boxes.length;
  while (low < high) {
    const mid = (low + high) >> 1;
    if (boxes[mid].top + boxes[mid].height < y) low = mid + 1;
    else high = mid;
  }
  return low;
}

/**
 * Pages à afficher : visibles, plus une marge au-dessus et en dessous pour
 * que le défilement ne montre pas de page vide. `null` si aucune.
 */
export function visibleRange(layout, scrollTop, viewportHeight, margin = viewportHeight) {
  const { boxes } = layout;
  const start = scrollTop - margin;
  const end = scrollTop + viewportHeight + margin;
  const first = firstEndingAfter(boxes, start);
  if (first >= boxes.length || boxes[first].top > end) return null;
  let last = first;
  while (last + 1 < boxes.length && boxes[last + 1].top <= end) last += 1;
  return { first, last };
}

/** Page courante : celle qui occupe le tiers haut de la zone visible. */
export function currentPage(layout, scrollTop, viewportHeight) {
  const { boxes } = layout;
  if (boxes.length === 0) return 0;
  const line = scrollTop + viewportHeight / 3;
  return Math.min(firstEndingAfter(boxes, line), boxes.length - 1);
}

/** Défilement qui amène le haut de la page `index` en haut de la zone visible. */
export function scrollTopForPage(layout, index) {
  const box = layout.boxes[Math.max(0, Math.min(index, layout.boxes.length - 1))];
  return box ? Math.max(0, box.top - PAGE_GAP) : 0;
}

/** Point de repère (page et fraction de page) pour garder la position au zoom. */
export function anchorAt(layout, scrollTop) {
  const page = Math.min(firstEndingAfter(layout.boxes, scrollTop), layout.boxes.length - 1);
  const box = layout.boxes[page];
  if (!box) return { page: 0, fraction: 0 };
  return { page, fraction: (scrollTop - box.top) / box.height };
}

export function scrollTopForAnchor(layout, anchor) {
  const box = layout.boxes[anchor.page];
  return box ? Math.max(0, box.top + anchor.fraction * box.height) : 0;
}

/**
 * Pixels par point demandés au moteur : zoom × densité de l'écran, bornés
 * par les limites du moteur ; au-delà, l'image est agrandie par le navigateur.
 */
export function renderScale(page, zoom, devicePixelRatio) {
  const wanted = zoom * CSS_PX_PER_POINT * devicePixelRatio;
  const byArea = Math.sqrt(MAX_RENDER_PIXELS / Math.max(page.width * page.height, 1));
  const scale = Math.min(wanted, MAX_RENDER_SCALE, byArea);
  return Math.max(0.05, Math.round(scale * 100) / 100);
}
