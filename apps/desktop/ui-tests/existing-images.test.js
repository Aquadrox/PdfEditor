// Images d'origine (E-06) : proposées à la sélection en mode Éditer,
// déplacées, supprimées ou remplacées ; rien n'est créé tant qu'on ne les
// modifie pas.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0, 0, 0], layout: 0, can_undo: false, can_redo: false, dirty: false };
const IMAGE = { object: 2, bounds: { x: 100, y: 142, width: 200, height: 100 } };
const ORIGINAL = { top_left: { x: 100, y: 142 }, width: 200, height: 100 };

function view(frame, id = 8) {
  const element = { kind: "existing_image", page: 0, object: 2, original: ORIGINAL, frame };
  const box = frame ?? ORIGINAL;
  return {
    elements: [{ id, element, bounds: { ...box.top_left, width: box.width, height: box.height } }],
    revisions: [1, 0, 0],
    layout: 0,
    can_undo: true,
    can_redo: false,
    dirty: true,
  };
}

async function openInEditMode(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    page_images: [{ ok: [IMAGE] }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator("#edit-toolbar:visible").waitFor();
  await original(app).waitFor();
  return app;
}

const original = (app) => app.page.locator('.page').first().locator('.element[data-id="orig:0:2"]');

async function waitForCall(app, command) {
  await app.page.waitForFunction((name) => window.__calls.some((c) => c.command === name), command);
  return app.callsTo(command);
}

const near = (a, b, tolerance = 1.5) => Math.abs(a - b) <= tolerance;

test("Image d'origine : choisie d'un clic, sans rien modifier ; panneau propre", async () => {
  const app = await openInEditMode();
  try {
    await original(app).click();
    await app.page.locator("#prop-fields:visible").waitFor();
    assert.equal(await app.page.textContent("#properties-title"), "Image du document");
    assert.equal(await app.page.locator("#prop-replace").isVisible(), true);
    assert.equal(await app.page.locator("#prop-duplicate").isVisible(), false);
    assert.equal(await app.page.inputValue("#prop-width"), "70,6");
    // Une simple sélection n'est pas une modification.
    assert.deepEqual(await app.callsTo("add_element"), []);
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), false);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Image d'origine glissée : son élément est créé avec le nouveau cadre", async () => {
  const moved = { top_left: { x: 150, y: 192 }, width: 200, height: 100 };
  const app = await openInEditMode({ add_element: [{ ok: { id: 8, view: view(moved) } }] });
  try {
    const box = await original(app).boundingBox();
    const ratio = box.width / 200;
    await app.page.mouse.move(box.x + box.width / 2, box.y + box.height / 2);
    await app.page.mouse.down();
    for (let i = 1; i <= 5; i += 1) await app.page.mouse.move(box.x + box.width / 2 + i * 10 * ratio, box.y + box.height / 2 + i * 10 * ratio);
    await app.page.mouse.up();
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.kind, e.page, e.object], ["existing_image", 0, 2]);
    assert.deepEqual(e.original, ORIGINAL);
    assert.ok(near(e.frame.top_left.x, 150, 6) && near(e.frame.top_left.y, 192, 6), JSON.stringify(e.frame));
    assert.deepEqual([e.frame.width, e.frame.height], [200, 100]);
    // L'élément créé remplace l'image proposée et reste sélectionné.
    await app.page.locator('.element[data-id="8"].selected').waitFor();
    assert.equal(await original(app).count(), 0);
  } finally {
    await app.close();
  }
});

test("Image d'origine supprimée : élément sans cadre, plus rien à sélectionner", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 8, view: view(null) } }] });
  try {
    await original(app).click();
    await app.page.click("#prop-delete");
    const [added] = await waitForCall(app, "add_element");
    assert.equal(added.element.kind, "existing_image");
    assert.equal(added.element.frame, null);
    await original(app).waitFor({ state: "detached" });
    assert.equal(await app.page.locator('.element[data-id="8"]').count(), 0, "image supprimée : pas de contour");
  } finally {
    await app.close();
  }
});

test("Remplacer : la nouvelle image remplit le cadre, l'ancienne est supprimée, en une opération", async () => {
  const imported = { image_id: 4, width: 50, height: 50 };
  const replaced = {
    ...view(null),
    elements: [
      ...view(null).elements,
      {
        id: 9,
        element: { kind: "image", page: 0, top_left: { x: 150, y: 142 }, width: 100, height: 100, image: 4, rotation: 0, opacity: 1, keep_ratio: true },
        bounds: { x: 150, y: 142, width: 100, height: 100 },
      },
    ],
  };
  const app = await openInEditMode({
    pick_image: [{ ok: imported }],
    edit_elements: [{ ok: { ids: [9, 8], view: replaced } }],
  });
  try {
    await original(app).click();
    await app.page.click("#prop-replace");
    const [call] = await waitForCall(app, "edit_elements");
    assert.deepEqual(call.changes, []);
    assert.deepEqual(call.removed, []);
    const [box, gone] = call.added;
    assert.deepEqual([box.kind, box.image, box.width, box.height], ["image", 4, 100, 100]);
    assert.deepEqual(box.top_left, { x: 150, y: 142 });
    assert.deepEqual([gone.kind, gone.object, gone.frame], ["existing_image", 2, null]);
    // La nouvelle image est sélectionnée.
    await app.page.locator('.element[data-id="9"].selected').waitFor();
  } finally {
    await app.close();
  }
});
