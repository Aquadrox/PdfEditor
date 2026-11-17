import assert from "node:assert/strict";
import { test } from "node:test";

import {
  TOOL_DEFAULTS,
  annotationResized,
  annotationWithGeometry,
  bboxOf,
  constrained,
  handlesOf,
  NOTE_SIZE,
  isAnnotation,
  newNote,
  newInk,
  newMarkup,
  newShape,
  translated,
  withAnnotationStyle,
  withPoint,
} from "./annotation-state.js";

const A4 = { width: 595, height: 842 };
const near = (a, b) => assert.ok(Math.abs(a - b) < 1e-6, `${a} ≠ ${b}`);

const rect = (start = { x: 100, y: 100 }, end = { x: 200, y: 150 }) =>
  newShape(0, start, end, TOOL_DEFAULTS.shape, A4);

test("forme : bornée à la page, ignorée si le glisser est trop court", () => {
  const r = newShape(1, { x: -10, y: 50 }, { x: 80, y: 900 }, { ...TOOL_DEFAULTS.shape, shape: "ellipse" }, A4);
  assert.deepEqual([r.kind, r.page, r.shape], ["shape", 1, "ellipse"]);
  assert.deepEqual([r.start, r.end], [{ x: 0, y: 50 }, { x: 80, y: 842 }]);
  assert.equal(newShape(0, { x: 10, y: 10 }, { x: 11, y: 11 }, TOOL_DEFAULTS.shape, A4), null);
  assert.ok(isAnnotation(r) && !isAnnotation({ kind: "text" }));
});

test("Maj : carré pour un cadre, angle de 45° pour un trait", () => {
  assert.deepEqual(constrained("rectangle", { x: 0, y: 0 }, { x: 30, y: -10 }), { x: 30, y: -30 });
  const end = constrained("line", { x: 0, y: 0 }, { x: 100, y: 10 });
  near(end.y, 0);
  near(end.x, Math.hypot(100, 10));
  const diagonal = constrained("arrow", { x: 0, y: 0 }, { x: 50, y: 45 });
  near(diagonal.x, diagonal.y);
});

test("trait à main levée : points trop proches écartés, bornés à la page", () => {
  let points = withPoint([], { x: 10, y: 10 }, A4);
  points = withPoint(points, { x: 10.5, y: 10.5 }, A4);
  points = withPoint(points, { x: 20, y: -5 }, A4);
  assert.deepEqual(points, [{ x: 10, y: 10 }, { x: 20, y: 0 }]);
  const ink = newInk(2, points, TOOL_DEFAULTS.ink);
  assert.deepEqual([ink.kind, ink.page, ink.width], ["ink", 2, 2]);
  // Un simple clic (pour désélectionner, par exemple) ne laisse rien.
  assert.equal(newInk(0, [{ x: 1, y: 1 }], TOOL_DEFAULTS.ink), null);
});

test("marquage : morceaux d'une même ligne réunis, lignes distinctes gardées", () => {
  const m = newMarkup(
    0,
    [
      { x: 100, y: 200, width: 40, height: 12 },
      { x: 141, y: 201, width: 30, height: 11 },
      { x: 100, y: 216, width: 50, height: 12 },
      { x: 0, y: 0, width: 0, height: 12 },
    ],
    TOOL_DEFAULTS.markup,
  );
  assert.equal(m.mark, "highlight");
  assert.deepEqual(m.areas, [
    { top_left: { x: 100, y: 200 }, width: 71, height: 12 },
    { top_left: { x: 100, y: 216 }, width: 50, height: 12 },
  ]);
  assert.equal(newMarkup(0, [], TOOL_DEFAULTS.markup), null);
});

test("déplacement : tous les points suivent, sans sortir de la page", () => {
  const moved = translated(rect(), 10, -500, A4);
  assert.deepEqual([moved.start, moved.end], [{ x: 110, y: 0 }, { x: 210, y: 50 }]);
  const ink = translated(newInk(0, [{ x: 1, y: 1 }, { x: 5, y: 9 }], TOOL_DEFAULTS.ink), 3, 3, A4);
  assert.deepEqual(ink.points, [{ x: 4, y: 4 }, { x: 8, y: 12 }]);
});

test("poignées : extrémités d'un trait, coins d'un cadre, aucune pour un marquage", () => {
  assert.deepEqual(handlesOf(rect()), ["nw", "ne", "sw", "se"]);
  assert.deepEqual(handlesOf({ ...rect(), shape: "arrow" }), ["start", "end"]);
  assert.deepEqual(handlesOf(newMarkup(0, [{ x: 0, y: 0, width: 5, height: 5 }], TOOL_DEFAULTS.markup)), []);
  const arrow = annotationResized({ ...rect(), shape: "arrow" }, "end", 20, 5, A4);
  assert.deepEqual([arrow.start, arrow.end], [{ x: 100, y: 100 }, { x: 220, y: 155 }]);
  // Coin bas droit tiré : le coin opposé reste en place, les points suivent.
  const big = annotationResized(rect(), "se", 100, 50, A4);
  assert.deepEqual(bboxOf(big), { x: 100, y: 100, width: 200, height: 100 });
  const ink = annotationResized(newInk(0, [{ x: 0, y: 0 }, { x: 10, y: 10 }, { x: 20, y: 0 }], TOOL_DEFAULTS.ink), "nw", 10, 5, A4);
  assert.deepEqual(ink.points, [{ x: 10, y: 5 }, { x: 15, y: 10 }, { x: 20, y: 5 }]);
});

test("position et taille saisies ; marquage : taille non modifiable", () => {
  const r = rect();
  assert.deepEqual(bboxOf(annotationWithGeometry(r, "x", 300, A4)), { x: 300, y: 100, width: 100, height: 50 });
  assert.deepEqual(bboxOf(annotationWithGeometry(r, "height", 80, A4)), { x: 100, y: 100, width: 100, height: 80 });
  const m = newMarkup(0, [{ x: 10, y: 10, width: 50, height: 10 }], TOOL_DEFAULTS.markup);
  assert.equal(annotationWithGeometry(m, "width", 80, A4), null);
  assert.deepEqual(annotationWithGeometry(m, "y", 30, A4).areas[0].top_left, { x: 10, y: 30 });
});

test("style : seules les propriétés de l'annotation changent", () => {
  const red = { r: 200, g: 0, b: 0 };
  const shape = withAnnotationStyle(rect(), { color: red, width: 6, filled: true, mark: "underline", size: 99 });
  assert.deepEqual([shape.color, shape.width, shape.filled, shape.mark, shape.size], [red, 6, true, undefined, undefined]);
  const m = withAnnotationStyle(newMarkup(0, [{ x: 0, y: 0, width: 5, height: 5 }], TOOL_DEFAULTS.markup), { mark: "strike_out", width: 4 });
  assert.deepEqual([m.mark, m.width], ["strike_out", undefined]);
});

test("note : centrée sur le clic, gardée dans la page, ni poignée ni taille", () => {
  const n = newNote(1, { x: 300, y: 5 }, A4, TOOL_DEFAULTS.note, "Camille");
  assert.deepEqual([n.kind, n.page, n.text, n.author], ["note", 1, "", "Camille"]);
  assert.deepEqual(n.top_left, { x: 290, y: 0 });
  assert.deepEqual(bboxOf(n), { x: 290, y: 0, width: NOTE_SIZE, height: NOTE_SIZE });
  assert.deepEqual(handlesOf(n), []);
  assert.equal(annotationWithGeometry(n, "width", 50, A4), null);
  assert.deepEqual(translated(n, 10, 10, A4).top_left, { x: 300, y: 10 });
  assert.deepEqual(withAnnotationStyle(n, { color: { r: 1, g: 2, b: 3 }, width: 9 }).width, undefined);
});

test("caviardage : zone tracée dans la page, texte réuni par ligne, poignées", async () => {
  const { newRedactArea, newRedaction, handlesOf, annotationWithGeometry, withAnnotationStyle } = await import("./annotation-state.js");
  const page = { width: 595, height: 842 };
  const area = newRedactArea(2, { x: 300, y: 50 }, { x: 100, y: 10 }, page);
  assert.deepEqual(area, { kind: "redact", page: 2, areas: [{ top_left: { x: 100, y: 10 }, width: 200, height: 40 }] });
  assert.equal(newRedactArea(0, { x: 10, y: 10 }, { x: 11, y: 40 }, page), null, "trop étroite");
  // Tracée hors de la page : ramenée dedans.
  assert.equal(newRedactArea(0, { x: 500, y: 800 }, { x: 900, y: 900 }, page).areas[0].width, 95);
  assert.deepEqual(handlesOf(area), ["nw", "ne", "sw", "se"]);
  assert.equal(annotationWithGeometry(area, "width", 50, page).areas[0].width, 50);
  // Deux morceaux sur une ligne, un autre en dessous : deux zones.
  const text = newRedaction(0, [
    { x: 72, y: 100, width: 40, height: 12 },
    { x: 112, y: 100, width: 30, height: 12 },
    { x: 72, y: 120, width: 50, height: 12 },
  ]);
  assert.equal(text.kind, "redact");
  assert.equal(text.areas.length, 2);
  assert.equal(text.areas[0].width, 70);
  assert.deepEqual(handlesOf(text), []);
  assert.equal(annotationWithGeometry(text, "height", 50, page), null);
  assert.equal(newRedaction(0, []), null);
  assert.deepEqual(withAnnotationStyle(area, { color: { r: 1, g: 2, b: 3 } }), area, "pas de style");
});
