import assert from "node:assert/strict";
import { test } from "node:test";

import { percentBox, scrollTopForRect, stretch } from "./text-geometry.js";

const page = { width: 200, height: 400 };

test("boîte placée en pourcentages de la page", () => {
  assert.deepEqual(percentBox({ x: 50, y: 100, width: 100, height: 20 }, page), {
    left: "25%",
    top: "25%",
    width: "50%",
    height: "5%",
  });
});

test("étirement du texte à la largeur du PDF", () => {
  assert.equal(stretch({ width: 120 }, 100), 1.2);
  // Mesure impossible (texte vide, police absente) : pas d'étirement.
  assert.equal(stretch({ width: 120 }, 0), 1);
  assert.equal(stretch({ width: 0 }, 50), 1);
});

test("défilement jusqu'à une occurrence", () => {
  // Page posée à 1000 px, affichée sur 800 px de haut : y = 100 pt → 200 px.
  const pageBox = { top: 1000, height: 800 };
  assert.equal(scrollTopForRect(pageBox, page, { y: 100 }, 600), 1000 + 200 - 200);
  assert.equal(scrollTopForRect({ top: 0, height: 800 }, page, { y: 0 }, 600), 0);
});
