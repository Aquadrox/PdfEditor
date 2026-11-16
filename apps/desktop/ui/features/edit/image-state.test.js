import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MIN_IMAGE_SIDE,
  isImagePath,
  newImageBox,
  resizedFrame,
  rotated,
  withOpacity,
  withSize,
} from "./image-state.js";

const A4 = { width: 595, height: 842 };
const near = (a, b) => assert.ok(Math.abs(a - b) < 1e-6, `${a} ≠ ${b}`);

function frame(x = 100, y = 100, width = 200, height = 100, keep = true) {
  return { kind: "image", page: 0, top_left: { x, y }, width, height, image: 1, rotation: 0, opacity: 1, keep_ratio: keep };
}

test("extensions d'image reconnues, majuscules comprises", () => {
  assert.ok(isImagePath("/photos/IMG_001.JPG"));
  assert.ok(isImagePath("C:\\scan.webp"));
  assert.ok(!isImagePath("/docs/contrat.pdf"));
  assert.ok(!isImagePath("sans-extension"));
});

test("nouvelle image : taille naturelle, centrée, réduite pour tenir dans la page", () => {
  const small = newImageBox(0, { image_id: 3, width: 200, height: 100 }, A4, { x: 300, y: 400 });
  assert.equal(small.image, 3);
  assert.deepEqual([small.width, small.height], [150, 75]);
  assert.deepEqual(small.top_left, { x: 225, y: 362.5 });
  // Photo de 4000 × 3000 : 60 % de la largeur au plus, proportions gardées.
  const photo = newImageBox(0, { image_id: 1, width: 4000, height: 3000 }, A4, { x: 0, y: 0 });
  near(photo.width, 595 * 0.6);
  near(photo.width / photo.height, 4 / 3);
  assert.deepEqual(photo.top_left, { x: 0, y: 0 });
});

test("coin tiré avec proportions gardées : le coin opposé ne bouge pas", () => {
  const f = frame();
  const se = resizedFrame(f, "se", 100, 0, A4);
  assert.deepEqual([se.width, se.height, se.top_left.x, se.top_left.y], [300, 150, 100, 100]);
  const nw = resizedFrame(f, "nw", 50, 999, A4);
  assert.deepEqual([nw.width, nw.height], [150, 75]);
  // Coin opposé (bas droite) fixe.
  near(nw.top_left.x + nw.width, 300);
  near(nw.top_left.y + nw.height, 200);
});

test("coin tiré sans proportions, borné à la page et à la taille minimale", () => {
  const f = frame(100, 100, 200, 100, false);
  const free = resizedFrame(f, "ne", 20, -30, A4);
  assert.deepEqual([free.width, free.height, free.top_left.y], [220, 130, 70]);
  const tiny = resizedFrame(f, "se", -500, -500, A4);
  assert.deepEqual([tiny.width, tiny.height], [MIN_IMAGE_SIDE, MIN_IMAGE_SIDE]);
  const out = resizedFrame(f, "se", 5000, 0, A4, true);
  assert.ok(out.top_left.x + out.width <= A4.width + 1e-9);
  near(out.width / out.height, 2);
});

test("rotation : quart de tour autour du centre, largeur et hauteur échangées", () => {
  const r = rotated(frame(), 1, A4);
  assert.deepEqual([r.rotation, r.width, r.height], [1, 100, 200]);
  assert.deepEqual(r.top_left, { x: 150, y: 50 });
  assert.equal(rotated(frame(), -1, A4).rotation, 3);
  assert.equal(rotated(rotated(r, 1, A4), 1, A4).rotation, 3);
});

test("taille saisie : l'autre côté suit si les proportions sont gardées", () => {
  const w = withSize(frame(), "width", 100, A4);
  assert.deepEqual([w.width, w.height], [100, 50]);
  const h = withSize(frame(100, 100, 200, 100, false), "height", 300, A4);
  assert.deepEqual([h.width, h.height], [200, 300]);
  const huge = withSize(frame(), "width", 5000, A4);
  assert.ok(huge.top_left.x + huge.width <= A4.width + 1e-9);
  near(huge.width / huge.height, 2);
});

test("opacité en %, bornée de 5 à 100", () => {
  assert.equal(withOpacity(frame(), "40").opacity, 0.4);
  assert.equal(withOpacity(frame(), 0).opacity, 0.05);
  assert.equal(withOpacity(frame(), "abc"), null);
});
