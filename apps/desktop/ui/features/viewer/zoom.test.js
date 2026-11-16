import assert from "node:assert/strict";
import { test } from "node:test";

import {
  CSS_PX_PER_POINT,
  FIT_MARGIN,
  MAX_ZOOM,
  ZOOM_STEPS,
  clampZoom,
  fitPage,
  fitWidth,
  formatZoom,
  zoomIn,
  zoomOut,
} from "./zoom.js";

test("les paliers montent et descendent", () => {
  assert.equal(zoomIn(1), 1.1);
  assert.equal(zoomOut(1), 0.9);
  // Entre deux paliers : on va au palier le plus proche dans le sens demandé.
  assert.equal(zoomIn(1.05), 1.1);
  assert.equal(zoomOut(1.05), 1);
});

test("les paliers s'arrêtent aux extrêmes", () => {
  assert.equal(zoomIn(MAX_ZOOM), MAX_ZOOM);
  assert.equal(zoomOut(ZOOM_STEPS[0]), ZOOM_STEPS[0]);
});

test("zoom borné et nombre invalide ramené à 100 %", () => {
  assert.equal(clampZoom(50), MAX_ZOOM);
  assert.equal(clampZoom(0), 0.1);
  assert.equal(clampZoom(Number.NaN), 1);
});

test("ajusté à la largeur : la page la plus large tient", () => {
  const pages = [{ width: 595, height: 842 }, { width: 842, height: 595 }];
  const zoom = fitWidth(pages, 1000);
  assert.ok(Math.abs(842 * CSS_PX_PER_POINT * zoom + 2 * FIT_MARGIN - 1000) < 1e-9);
});

test("page entière : limitée par la hauteur pour une page A4", () => {
  const zoom = fitPage({ width: 595, height: 842 }, 1200, 700);
  assert.ok(Math.abs(842 * CSS_PX_PER_POINT * zoom + 2 * FIT_MARGIN - 700) < 1e-9);
});

test("affichage en pourcentage", () => {
  assert.equal(formatZoom(1.1), "110 %");
  assert.equal(formatZoom(0.333), "33 %");
});
