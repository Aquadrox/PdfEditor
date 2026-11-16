import assert from "node:assert/strict";
import { test } from "node:test";

import {
  MIN_TEXT_WIDTH,
  colorFromHex,
  duplicated,
  geometryInMm,
  cssFontFamily,
  hexFromColor,
  mmFromPt,
  moved,
  newTextBox,
  nudged,
  parseSize,
  pagePoint,
  pointsFromPixels,
  ptFromMm,
  resized,
  withGeometryMm,
  withStyle,
} from "./edit-state.js";

const A4 = { width: 595, height: 842 };
const STYLE = { font: "sans", size: 11, bold: false, italic: false, color: { r: 0, g: 0, b: 0 }, align: "left", line_height: 1.25 };
const box = (x, y, width = 200) => ({ kind: "text", page: 0, top_left: { x, y }, width, text: "a", style: STYLE });

test("millimètres et points", () => {
  assert.equal(mmFromPt(595), 209.9);
  assert.ok(Math.abs(ptFromMm(210) - 595.28) < 0.01);
});

test("clic sur le cadre affiché → point de la page", () => {
  // Page de 595 pt affichée sur 1190 px : 2 px par point.
  assert.deepEqual(pagePoint(144, 200, 1190, 1684, A4), { x: 72, y: 100 });
  assert.deepEqual(pointsFromPixels(20, -10, 1190, A4), { dx: 10, dy: -5 });
});

test("nouvelle zone gardée dans la page", () => {
  const near = newTextBox(1, { x: 72, y: 100 }, A4, STYLE);
  assert.equal(near.page, 1);
  assert.deepEqual(near.top_left, { x: 72, y: 100 });
  assert.equal(near.text, "");
  // Clic près du bord droit : la zone est décalée pour tenir.
  const edge = newTextBox(0, { x: 580, y: 840 }, A4, STYLE);
  assert.equal(edge.top_left.x + edge.width, 595 - 4);
  assert.ok(edge.top_left.y <= 842 - 11);
});

test("déplacement borné par la page", () => {
  assert.deepEqual(moved(box(72, 100), 10, 20, A4).top_left, { x: 82, y: 120 });
  assert.deepEqual(moved(box(72, 100), -500, -500, A4).top_left, { x: 0, y: 0 });
  assert.equal(moved(box(72, 100), 1000, 0, A4).top_left.x, 595 - 200);
});

test("redimensionnement par la droite et par la gauche", () => {
  assert.equal(resized(box(72, 100), "right", 50, A4).width, 250);
  const left = resized(box(72, 100), "left", 22, A4);
  assert.equal(left.top_left.x, 94);
  assert.equal(left.width, 178);
  // Jamais plus étroit que le minimum, jamais hors de la page.
  assert.equal(resized(box(72, 100), "right", -500, A4).width, MIN_TEXT_WIDTH);
  assert.equal(resized(box(72, 100), "left", 500, A4).width, MIN_TEXT_WIDTH);
  assert.equal(resized(box(500, 100, 50), "right", 200, A4).width, 95);
});

test("flèches du clavier", () => {
  assert.deepEqual(nudged(box(72, 100), "ArrowRight", false, A4).top_left, { x: 73, y: 100 });
  assert.deepEqual(nudged(box(72, 100), "ArrowUp", true, A4).top_left, { x: 72, y: 90 });
  assert.equal(nudged(box(72, 100), "Enter", false, A4), null);
});

test("style modifié sans toucher au reste", () => {
  const bold = withStyle(box(72, 100), { bold: true, size: 14 });
  assert.equal(bold.style.bold, true);
  assert.equal(bold.style.size, 14);
  assert.equal(bold.style.font, "sans");
  assert.equal(STYLE.bold, false, "original intact");
});

test("couleurs et polices d'affichage", () => {
  assert.equal(hexFromColor({ r: 39, g: 71, b: 184 }), "#2747b8");
  assert.deepEqual(colorFromHex("#8A1C2B"), { r: 138, g: 28, b: 43 });
  assert.equal(colorFromHex("rouge"), null);
  assert.match(cssFontFamily("serif"), /Times/);
});

test("corps saisi : virgule acceptée, demi-points, bornes de pde-core", () => {
  assert.equal(parseSize("12,5"), 12.5);
  assert.equal(parseSize("11.3"), 11.5);
  assert.equal(parseSize("1"), 4);
  assert.equal(parseSize("999"), 200);
  assert.equal(parseSize("abc"), null);
});

test("dupliquer décale la copie sans sortir de la page", () => {
  const box = newTextBox(0, { x: 100, y: 100 }, A4, STYLE);
  assert.deepEqual(duplicated(box, A4).top_left, { x: 112, y: 112 });
  const corner = { ...box, top_left: { x: A4.width - box.width, y: A4.height - STYLE.size } };
  assert.deepEqual(duplicated(corner, A4).top_left, corner.top_left);
});

test("position et largeur en mm, aller et retour", () => {
  const box = { ...newTextBox(0, { x: 0, y: 0 }, A4, STYLE), top_left: { x: ptFromMm(20), y: ptFromMm(30) } };
  assert.deepEqual(geometryInMm(box, { height: ptFromMm(5) }), { x: 20, y: 30, width: 70.6, height: 5 });
  assert.ok(Math.abs(withGeometryMm(box, "x", "25,5", A4).top_left.x - ptFromMm(25.5)) < 1e-9);
  assert.ok(Math.abs(withGeometryMm(box, "y", "40", A4).top_left.y - ptFromMm(40)) < 1e-9);
  assert.ok(Math.abs(withGeometryMm(box, "width", "50", A4).width - ptFromMm(50)) < 1e-9);
  assert.equal(withGeometryMm(box, "x", "", A4), null);
  // Hors de la page : ramené au bord.
  assert.equal(withGeometryMm(box, "x", "500", A4).top_left.x, A4.width - box.width);
});
