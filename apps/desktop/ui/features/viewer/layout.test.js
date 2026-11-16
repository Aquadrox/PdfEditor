import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DESK_PADDING,
  MAX_RENDER_SCALE,
  PAGE_GAP,
  anchorAt,
  computeLayout,
  currentPage,
  renderScale,
  scrollTopForAnchor,
  scrollTopForPage,
  visibleRange,
} from "./layout.js";

// 72 pt × 0,75 = 54 pt → 72 px CSS : calculs ronds.
const page = { width: 72, height: 72 };
const layout = computeLayout([page, page, page, page], 0.75);

test("les pages s'empilent avec un espace", () => {
  assert.deepEqual(
    layout.boxes.map((b) => b.top),
    [DESK_PADDING, DESK_PADDING + 72 + PAGE_GAP, DESK_PADDING + 2 * 88, DESK_PADDING + 3 * 88],
  );
  assert.equal(layout.boxes[0].width, 72);
  assert.equal(layout.totalHeight, DESK_PADDING + 4 * 72 + 3 * PAGE_GAP + DESK_PADDING);
});

test("pages visibles avec et sans marge", () => {
  // Zone de 100 px depuis le haut : pages 0 et 1 (la page 1 commence à 116).
  assert.deepEqual(visibleRange(layout, 0, 120, 0), { first: 0, last: 1 });
  assert.deepEqual(visibleRange(layout, 200, 50, 0), { first: 2, last: 2 });
  assert.deepEqual(visibleRange(layout, 200, 50, 90), { first: 1, last: 3 });
  // Bas de la page 0 exactement au début de la marge : elle compte encore.
  assert.deepEqual(visibleRange(layout, 200, 50, 100), { first: 0, last: 3 });
  assert.equal(visibleRange(layout, 5000, 50, 0), null);
  assert.equal(visibleRange(computeLayout([], 1), 0, 100), null);
});

test("page courante selon le défilement", () => {
  assert.equal(currentPage(layout, 0, 90), 0);
  assert.equal(currentPage(layout, 116, 90), 1);
  assert.equal(currentPage(layout, 10_000, 90), 3);
  assert.equal(currentPage(computeLayout([], 1), 0, 90), 0);
});

test("aller à une page", () => {
  assert.equal(scrollTopForPage(layout, 0), DESK_PADDING - PAGE_GAP);
  assert.equal(scrollTopForPage(layout, 2), layout.boxes[2].top - PAGE_GAP);
  assert.equal(scrollTopForPage(layout, 99), layout.boxes[3].top - PAGE_GAP);
});

test("la position est gardée quand le zoom change", () => {
  const anchor = anchorAt(layout, layout.boxes[2].top + 36); // milieu de la page 2
  assert.deepEqual(anchor, { page: 2, fraction: 0.5 });
  const bigger = computeLayout([page, page, page, page], 1.5);
  assert.equal(scrollTopForAnchor(bigger, anchor), bigger.boxes[2].top + 72);
});

test("zoom de rendu : densité de l'écran et limites du moteur", () => {
  const a4 = { width: 595, height: 842 };
  assert.equal(renderScale(a4, 1, 1), 1.33);
  assert.equal(renderScale(a4, 1, 2), 2.67);
  // Très fort zoom : borné par la surface maximale de l'image.
  assert.ok(renderScale(a4, 8, 2) < MAX_RENDER_SCALE);
  assert.ok(595 * 842 * renderScale(a4, 8, 2) ** 2 <= 40_000_000 * 1.01);
  // Petite page : bornée par le zoom maximal du moteur.
  assert.equal(renderScale({ width: 10, height: 10 }, 8, 2), MAX_RENDER_SCALE);
});
