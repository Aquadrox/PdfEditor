// Sélection de plusieurs éléments (E-03) : Maj+clic, Ctrl+A, Tab,
// opérations groupées, aligner, répartir, magnétisme.

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

function image(x, y, width, height, page = 0) {
  return { kind: "image", page, top_left: { x, y }, width, height, image: 1, rotation: 0, opacity: 1, keep_ratio: true };
}

const A = image(100, 100, 100, 50);
const B = image(300, 200, 80, 40);
const C = image(150, 400, 60, 30);
const OTHER_PAGE = image(50, 50, 40, 40, 1);

function view(elements) {
  return {
    elements: elements.map(([id, element]) => ({ id, element, bounds: { ...element.top_left, width: element.width, height: element.height } })),
    revisions: [1, 1, 0],
    can_undo: true,
    can_redo: false,
    dirty: true,
  };
}

const ALL = view([[1, A], [2, B], [3, C], [4, OTHER_PAGE]]);

async function openInEditMode(commands = {}, settings = DEFAULT_SETTINGS) {
  const app = await launch({
    get_settings: [{ ok: { settings, warnings: [] } }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: ALL }],
    update_elements: [{ ok: ALL }],
    update_element: [{ ok: ALL }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator(".edit-layer .element").first().waitFor();
  return app;
}

async function waitForCall(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

const box = (app, id) => app.page.locator(`.edit-layer .element[data-id="${id}"]`);

async function screenPoint(app, x, y) {
  const layer = await app.page.locator(".page .edit-layer").first().boundingBox();
  return { x: layer.x + (x / 595) * layer.width, y: layer.y + (y / 842) * layer.height };
}

async function drag(app, from, to, { alt = false } = {}) {
  const a = await screenPoint(app, from.x, from.y);
  const b = await screenPoint(app, to.x, to.y);
  await app.page.mouse.move(a.x, a.y);
  if (alt) await app.page.keyboard.down("Alt");
  await app.page.mouse.down();
  await app.page.mouse.move((a.x + b.x) / 2, (a.y + b.y) / 2, { steps: 3 });
  await app.page.mouse.move(b.x, b.y, { steps: 3 });
  await app.page.mouse.up();
  if (alt) await app.page.keyboard.up("Alt");
}

const near = (a, b, tolerance = 0.5) => Math.abs(a - b) <= tolerance;

test("Maj+clic : deux éléments sélectionnés, panneau d'alignement, Aligner à gauche", async () => {
  const app = await openInEditMode();
  try {
    await box(app, 1).click();
    await box(app, 2).click({ modifiers: ["Shift"] });
    assert.equal(await app.page.locator(".edit-layer .element.selected").count(), 2);
    assert.equal(await app.page.locator(".edit-layer .handle").count(), 0, "pas de poignées à plusieurs");
    assert.equal(await app.page.textContent("#properties-title"), "2 éléments");
    assert.equal(await app.page.locator("#prop-geometry").isVisible(), false);
    assert.equal(await app.page.isDisabled('[data-arrange="distribute-x"]'), true, "répartir : trois au moins");
    // Seul B bouge : une modification simple.
    await app.page.click('[data-arrange="left"]');
    const [call] = await waitForCall(app, "update_element");
    assert.deepEqual([call.elementId, call.element.top_left.x], [2, 100]);

    // Maj+clic sur un élément sélectionné : il sort de la sélection.
    await box(app, 2).click({ modifiers: ["Shift"] });
    assert.equal(await app.page.locator(".edit-layer .element.selected").count(), 1);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("déplacer à plusieurs : tous les éléments suivent, en une seule opération", async () => {
  const app = await openInEditMode();
  try {
    await box(app, 1).click();
    await box(app, 2).click({ modifiers: ["Shift"] });
    await drag(app, { x: 150, y: 125 }, { x: 170, y: 155 }, { alt: true });
    const [call] = await waitForCall(app, "update_elements");
    const moved = Object.fromEntries(call.changes.map((c) => [c.elementId, c.element.top_left]));
    assert.ok(near(moved[1].x, 120, 1) && near(moved[1].y, 130, 1), JSON.stringify(moved));
    assert.ok(near(moved[2].x - moved[1].x, 200, 0.01) && near(moved[2].y - moved[1].y, 100, 0.01), "écart gardé");
  } finally {
    await app.close();
  }
});

test("Suppr, Dupliquer et flèches agissent sur toute la sélection", async () => {
  const app = await openInEditMode({
    add_elements: [{ ok: { ids: [5, 6], view: ALL } }],
    remove_elements: [{ ok: view([[3, C], [4, OTHER_PAGE]]) }],
  });
  try {
    await box(app, 1).click();
    await box(app, 2).click({ modifiers: ["Shift"] });
    await app.page.click("#prop-duplicate");
    const [dup] = await waitForCall(app, "add_elements");
    assert.deepEqual(dup.elements.map((e) => e.top_left), [{ x: 112, y: 112 }, { x: 312, y: 212 }]);

    await box(app, 1).click();
    await box(app, 2).click({ modifiers: ["Shift"] });
    await app.page.locator("#desk").focus();
    await app.page.keyboard.press("ArrowRight");
    const [nudge] = await waitForCall(app, "update_elements");
    assert.deepEqual(nudge.changes.map((c) => c.element.top_left.x), [101, 301]);

    await app.page.keyboard.press("Delete");
    const [removed] = await waitForCall(app, "remove_elements");
    assert.deepEqual(removed.elementIds.sort(), [1, 2]);
  } finally {
    await app.close();
  }
});

test("Tab, Maj+Tab : élément suivant ou précédent ; Ctrl+A : toute la page", async () => {
  const app = await openInEditMode();
  try {
    await app.page.locator("#desk").focus();
    await app.page.keyboard.press("Tab");
    assert.equal(await box(app, 1).getAttribute("aria-pressed"), "true", "ordre de lecture : A d'abord");
    await app.page.keyboard.press("Tab");
    assert.equal(await box(app, 2).getAttribute("aria-pressed"), "true");
    await app.page.keyboard.press("Shift+Tab");
    assert.equal(await box(app, 1).getAttribute("aria-pressed"), "true");
    await app.page.keyboard.press("Control+a");
    assert.equal(await app.page.textContent("#properties-title"), "3 éléments", "page 1 seulement");
    // A (100 à 200), C (150 à 210), B (300 à 380) : écarts de 20, C va à 220.
    await app.page.click('[data-arrange="distribute-x"]');
    const [call] = await waitForCall(app, "update_element");
    assert.deepEqual([call.elementId, call.element.top_left.x], [3, 220]);
  } finally {
    await app.close();
  }
});

test("magnétisme : bord calé sur un autre élément avec repère ; Alt le désactive", async () => {
  const app = await openInEditMode();
  try {
    // B (x = 300) amené près du bord gauche de C (x = 150) : calé à 150.
    await box(app, 2).click();
    const a = await screenPoint(app, 340, 220);
    const b = await screenPoint(app, 192, 220);
    await app.page.mouse.move(a.x, a.y);
    await app.page.mouse.down();
    await app.page.mouse.move(b.x, b.y, { steps: 6 });
    assert.ok((await app.page.locator(".snap-guide").count()) >= 1, "repère affiché");
    await app.page.mouse.up();
    assert.equal(await app.page.locator(".snap-guide").count(), 0, "repère retiré");
    const [snappedCall] = await waitForCall(app, "update_element");
    assert.equal(snappedCall.element.top_left.x, 150);

    await drag(app, { x: 340, y: 220 }, { x: 192, y: 220 }, { alt: true });
    const [, free] = await waitForCall(app, "update_element", 2);
    assert.ok(!near(free.element.top_left.x, 150, 0.2), `${free.element.top_left.x}`);
  } finally {
    await app.close();
  }
});

test("grille réglée : un déplacement sans repère proche tombe sur la grille", async () => {
  const settings = structuredClone(DEFAULT_SETTINGS);
  settings.edit.grid_mm = 10;
  const app = await openInEditMode({}, settings);
  try {
    await box(app, 3).click();
    // Destination loin de tout repère (bords et centres des éléments, de la page).
    await drag(app, { x: 180, y: 415 }, { x: 240, y: 503 });
    const [call] = await waitForCall(app, "update_element");
    const step = (10 * 72) / 25.4;
    const { x, y } = call.element.top_left;
    assert.ok(near(x / step, Math.round(x / step), 1e-6) && near(y / step, Math.round(y / step), 1e-6), JSON.stringify({ x, y }));
  } finally {
    await app.close();
  }
});

test("un élément non sélectionné se déplace dès le premier glisser, sans erreur", async () => {
  const app = await openInEditMode();
  try {
    await drag(app, { x: 150, y: 125 }, { x: 170, y: 160 }, { alt: true });
    const [call] = await waitForCall(app, "update_element");
    assert.equal(call.elementId, 1);
    assert.ok(near(call.element.top_left.x, 120, 1) && near(call.element.top_left.y, 135, 1), JSON.stringify(call.element.top_left));
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Ctrl+A après un clic sur un outil : les éléments, pas le texte de la fenêtre", async () => {
  const app = await openInEditMode();
  try {
    await app.page.click("#tool-text");
    await app.page.keyboard.press("Control+a");
    assert.equal(await app.page.textContent("#properties-title"), "3 éléments");
    assert.equal(await app.page.evaluate(() => getSelection().isCollapsed), true);
  } finally {
    await app.close();
  }
});
