import assert from "node:assert/strict";
import { test } from "node:test";

import { clampWidth } from "./panel-resize.js";

test("largeur bornée entre le minimum et 70 % de la fenêtre", () => {
  assert.equal(clampWidth(500, 300, 1280), 500);
  assert.equal(clampWidth(100, 300, 1280), 300);
  assert.equal(clampWidth(2000, 300, 1280), 896);
  // Fenêtre étroite : le minimum l'emporte.
  assert.equal(clampWidth(900, 300, 360), 300);
});
