import assert from "node:assert/strict";
import { test } from "node:test";

import { aligned, distributed, nextInOrder, readingOrder, samePage, snapped, union } from "./selection-state.js";

const A4 = { width: 595, height: 842 };

/** Élément « image » minimal (déplacé par moved) avec sa place. */
function item(id, x, y, width = 50, height = 20, page = 0) {
  return {
    id,
    element: { kind: "image", page, top_left: { x, y }, width, height, image: 1, rotation: 0, opacity: 1, keep_ratio: true },
    bounds: { x, y, width, height },
  };
}

const xs = (changes) => Object.fromEntries(changes.map((c) => [c.id, c.element.top_left.x]));
const ys = (changes) => Object.fromEntries(changes.map((c) => [c.id, c.element.top_left.y]));

test("ordre de lecture et Tab en boucle, Maj+Tab à rebours", () => {
  const items = [item(1, 300, 100), item(2, 100, 100), item(3, 100, 50), item(4, 0, 0, 10, 10, 1)];
  assert.deepEqual(readingOrder(items).map((i) => i.id), [3, 2, 1, 4]);
  assert.equal(nextInOrder(items, null), 3);
  assert.equal(nextInOrder(items, 1), 4);
  assert.equal(nextInOrder(items, 4), 3);
  assert.equal(nextInOrder(items, 3, true), 4);
  assert.equal(nextInOrder([], null), null);
});

test("cadre commun, même page", () => {
  assert.deepEqual(union([item(1, 10, 20).bounds, item(2, 100, 5, 30, 10).bounds]), { x: 10, y: 5, width: 120, height: 35 });
  assert.ok(samePage([item(1, 0, 0), item(2, 5, 5)]));
  assert.ok(!samePage([item(1, 0, 0), item(2, 5, 5, 10, 10, 1)]));
});

test("aligner : bords et centres du cadre commun, seuls les éléments qui bougent", () => {
  const items = [item(1, 100, 100, 50), item(2, 200, 150, 100), item(3, 150, 300, 20)];
  assert.deepEqual(xs(aligned(items, "left", A4)), { 2: 100, 3: 100 });
  assert.deepEqual(xs(aligned(items, "right", A4)), { 1: 250, 3: 280 });
  assert.deepEqual(xs(aligned(items, "center", A4)), { 1: 175, 2: 150, 3: 190 });
  assert.deepEqual(ys(aligned(items, "top", A4)), { 2: 100, 3: 100 });
  assert.deepEqual(ys(aligned(items, "bottom", A4)), { 1: 300, 2: 300 });
  assert.deepEqual(ys(aligned(items, "middle", A4)), { 1: 200, 2: 200, 3: 200 });
});

test("répartir : écarts égaux, extrêmes en place, à partir de trois", () => {
  const items = [item(1, 0, 0, 20), item(2, 30, 0, 20), item(3, 200, 0, 20)];
  // Étendue 0 → 220, 60 de remplis : écart de 80.
  assert.deepEqual(xs(distributed(items, "x", A4)), { 2: 100 });
  assert.deepEqual(distributed(items.slice(0, 2), "x", A4), []);
  const vertical = [item(1, 0, 0, 10, 10), item(2, 0, 15, 10, 10), item(3, 0, 100, 10, 10)];
  assert.deepEqual(ys(distributed(vertical, "y", A4)), { 2: 50 });
});

test("magnétisme : bord d'un autre élément, centre de la page, sinon grille", () => {
  const box = { x: 100, y: 100, width: 50, height: 20 };
  const other = { x: 203, y: 400, width: 40, height: 40 };
  // Bord gauche amené à 2 pt du bord gauche de l'autre : calé dessus.
  const s = snapped(box, 101, 0, [other], A4, 4);
  assert.equal(s.dx, 103);
  assert.deepEqual(s.guides, [{ axis: "x", at: 203 }]);
  // Centre vertical près du centre de la page (421).
  const c = snapped(box, 0, 309, [], A4, 4);
  assert.equal(c.dy, 311);
  assert.deepEqual(c.guides, [{ axis: "y", at: 421 }]);
  // Rien de proche : grille de 10 pt sur le bord gauche et le haut.
  const g = snapped(box, 27, 33, [], A4, 2, 10);
  assert.deepEqual([g.dx, g.dy, g.guides], [30, 30, []]);
  // Sans grille ni repère : déplacement inchangé.
  assert.deepEqual(snapped(box, 27, 33, [], A4, 2), { dx: 27, dy: 33, guides: [] });
});
