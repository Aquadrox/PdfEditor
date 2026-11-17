import assert from "node:assert/strict";
import { test } from "node:test";

import {
  clamped,
  clicked,
  dropIndex,
  insertionPoint,
  isStill,
  moved,
  selectAll,
  selectOnly,
  selectPages,
} from "./page-selection.js";

test("clic seul, Ctrl+clic qui ajoute puis retire, Maj+clic en plage", () => {
  let s = clicked(selectOnly(0), 3);
  assert.deepEqual(s.pages, [3]);
  s = clicked(s, 1, { toggle: true });
  assert.deepEqual(s.pages, [1, 3]);
  s = clicked(s, 3, { toggle: true });
  assert.deepEqual(s.pages, [1]);
  // Maj+clic : de l'ancre (dernier clic, Ctrl compris) à la page.
  assert.deepEqual(clicked(s, 4, { shift: true }).pages, [3, 4]);
  s = clicked(selectOnly(1), 4, { shift: true });
  assert.deepEqual(s.pages, [1, 2, 3, 4]);
  s = clicked(s, 0, { shift: true });
  assert.deepEqual(s.pages, [0, 1]);
  // Ctrl+Maj : la plage s'ajoute.
  s = clicked(selectPages([5]), 7, { shift: true, toggle: true });
  assert.deepEqual(s.pages, [5, 6, 7]);
});

test("tout sélectionner et sélection donnée par une opération", () => {
  assert.deepEqual(selectAll(3).pages, [0, 1, 2]);
  assert.deepEqual(selectAll(0).pages, []);
  assert.deepEqual(selectPages([3, 1, 3]), { pages: [1, 3], anchor: 1, focus: 3 });
});

test("flèches dans la grille, bornées ; Maj étend depuis l'ancre", () => {
  const s = selectOnly(4);
  assert.deepEqual(moved(s, "ArrowRight", 10, 4).pages, [5]);
  assert.deepEqual(moved(s, "ArrowUp", 10, 4).pages, [0]);
  assert.deepEqual(moved(s, "ArrowDown", 10, 4).pages, [8]);
  assert.deepEqual(moved(selectOnly(8), "ArrowDown", 10, 4).pages, [9]);
  assert.deepEqual(moved(selectOnly(0), "ArrowLeft", 10, 4).pages, [0]);
  assert.deepEqual(moved(s, "Home", 10, 4).pages, [0]);
  assert.deepEqual(moved(s, "End", 10, 4).pages, [9]);
  assert.deepEqual(moved(s, "ArrowRight", 10, 4, true).pages, [4, 5]);
  assert.equal(moved(s, "a", 10, 4), s);
});

test("après une suppression, la sélection reste dans le document", () => {
  assert.deepEqual(clamped(selectPages([1, 5]), 3).pages, [1]);
  assert.deepEqual(clamped(selectOnly(5), 3).pages, [2]);
  assert.deepEqual(clamped(selectPages([0, 1]), 2).pages, [0, 1]);
});

test("place de dépôt d'après le pointeur", () => {
  // Deux rangées de trois cartes 100 × 140, espacées de 20.
  const boxes = [0, 1, 2, 3, 4].map((i) => ({
    left: (i % 3) * 120,
    top: Math.floor(i / 3) * 160,
    width: 100,
    height: 140,
  }));
  assert.equal(dropIndex(boxes, 10, 50), 0, "moitié gauche de la première");
  assert.equal(dropIndex(boxes, 80, 50), 1, "moitié droite de la première");
  assert.equal(dropIndex(boxes, 350, 50), 3, "après la fin de la rangée");
  assert.equal(dropIndex(boxes, 150, 200), 4, "deuxième rangée");
  assert.equal(dropIndex(boxes, 300, 200), 5, "après la dernière page");
  assert.equal(dropIndex(boxes, 10, 145), 0, "entre deux rangées : la plus proche");
  assert.equal(dropIndex([], 0, 0), 0);
});

test("dépôt sans effet, et place d'une page blanche", () => {
  assert.equal(isStill([2, 3], 2), true);
  assert.equal(isStill([2, 3], 4), true);
  assert.equal(isStill([2, 3], 5), false);
  assert.equal(isStill([1, 3], 2), false, "pages non contiguës : regroupées");
  assert.equal(insertionPoint(selectPages([1, 4]), 6), 5);
  assert.equal(insertionPoint({ pages: [], anchor: null, focus: null }, 6), 6);
});
