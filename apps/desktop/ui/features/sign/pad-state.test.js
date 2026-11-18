import assert from "node:assert/strict";
import { test } from "node:test";

import { clearPaper, contain, fittingSize, inkBounds } from "./pad-state.js";

function image(width, height, inked = []) {
  const data = new Uint8ClampedArray(width * height * 4);
  for (const [x, y] of inked) data[(y * width + x) * 4 + 3] = 255;
  return data;
}

test("Encre détourée avec une marge bornée à l'image", () => {
  assert.equal(inkBounds(image(20, 10), 20, 10), null);
  assert.deepEqual(inkBounds(image(20, 10, [[5, 4], [12, 6]]), 20, 10, 2), { x: 3, y: 2, width: 12, height: 7 });
  assert.deepEqual(inkBounds(image(20, 10, [[0, 0], [19, 9]]), 20, 10, 8), { x: 0, y: 0, width: 20, height: 10 });
});

test("Papier clair rendu transparent, encre gardée", () => {
  const data = new Uint8ClampedArray([255, 255, 255, 255, 20, 30, 90, 255, 222, 222, 222, 255]);
  clearPaper(data);
  assert.equal(data[3], 0);
  assert.equal(data[7], 255);
  assert.ok(data[11] > 0 && data[11] < 255, `fondu : ${data[11]}`);
});

test("Ajustement et corps de police", () => {
  assert.deepEqual(contain(400, 100, 200, 100), { x: 0, y: 25, width: 200, height: 50 });
  assert.equal(fittingSize((size) => size * 5, 400), 80);
  assert.equal(fittingSize((size) => size * 50, 100), 24);
});
