import assert from "node:assert/strict";
import { test } from "node:test";

import {
  DOUBLE_TAP_FACTOR,
  DOUBLE_TAP_MS,
  distance,
  doubleTapTarget,
  isDoubleTap,
  midpoint,
  pinchZoom,
  scrollLeftForFocus,
} from "./touch-zoom.js";
import { MAX_ZOOM, MIN_ZOOM } from "./zoom.js";

test("distance et milieu de deux doigts", () => {
  assert.equal(distance({ x: 0, y: 0 }, { x: 30, y: 40 }), 50);
  assert.deepEqual(midpoint({ x: 10, y: 20 }, { x: 30, y: 60 }), { x: 20, y: 40 });
});

test("pincer : le zoom suit l'écartement des doigts, borné", () => {
  assert.equal(pinchZoom(1, 100, 200), 2);
  assert.equal(pinchZoom(1.5, 200, 100), 0.75);
  assert.equal(pinchZoom(4, 10, 1000), MAX_ZOOM);
  assert.equal(pinchZoom(0.5, 1000, 1), MIN_ZOOM);
  // Doigts confondus au départ : rien ne change.
  assert.equal(pinchZoom(1.2, 0, 50), 1.2);
});

test("double tapotement : rapproché dans le temps et l'espace", () => {
  const first = { x: 100, y: 100, time: 1000 };
  assert.equal(isDoubleTap(null, first), false);
  assert.equal(isDoubleTap(first, { x: 110, y: 105, time: 1000 + DOUBLE_TAP_MS - 1 }), true);
  assert.equal(isDoubleTap(first, { x: 110, y: 105, time: 1000 + DOUBLE_TAP_MS + 50 }), false);
  assert.equal(isDoubleTap(first, { x: 200, y: 100, time: 1100 }), false);
});

test("double tapotement : grossit depuis la largeur, y revient sinon", () => {
  assert.deepEqual(doubleTapTarget(0.5, "width", 0.5), { zoom: 0.5 * DOUBLE_TAP_FACTOR, fit: null });
  // Zoom libre mais égal à la largeur : comme ajusté.
  assert.deepEqual(doubleTapTarget(0.5, null, 0.5), { zoom: 1, fit: null });
  assert.deepEqual(doubleTapTarget(1.3, null, 0.5), { zoom: 0.5, fit: "width" });
  assert.equal(doubleTapTarget(6, "width", 6).zoom, MAX_ZOOM);
});

test("défilement horizontal : le point sous les doigts reste en place", () => {
  // Contenu centré de 400 px dans une table de 400 px ; zoom ×2 au point 300.
  const left = scrollLeftForFocus({
    scroll: 0,
    from: 300,
    to: 300,
    ratio: 2,
    before: { offset: 0, size: 400 },
    after: { offset: 0, size: 800 },
  });
  // Le point (100 px à droite du centre) est à 200 px du centre (400) après : 600 − 300.
  assert.equal(left, 300);
  // Les doigts ont glissé de 50 px vers la gauche : la page suit.
  const moved = scrollLeftForFocus({
    scroll: 0,
    from: 300,
    to: 250,
    ratio: 2,
    before: { offset: 0, size: 400 },
    after: { offset: 0, size: 800 },
  });
  assert.equal(moved, 350);
  // Jamais négatif (la table ne défile pas avant son bord).
  assert.equal(
    scrollLeftForFocus({ scroll: 0, from: 10, to: 300, ratio: 1, before: { offset: 0, size: 400 }, after: { offset: 0, size: 400 } }),
    0,
  );
});
