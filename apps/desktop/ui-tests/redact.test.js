// Caviardage (E-07) : outil Caviarder (texte sélectionné ou zone tracée),
// panneau, confirmation avant d'écrire le contenu supprimé.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };
const RUN = { text: "Bonjour PdfEditor", x: 72, y: 60, width: 200, height: 14 };
const ZONE = { kind: "redact", page: 0, areas: [{ top_left: { x: 100, y: 200 }, width: 200, height: 100 }] };

function view(element = ZONE, id = 5) {
  return {
    elements: [{ id, element, bounds: { x: 100, y: 200, width: 200, height: 100 } }],
    revisions: [1, 0, 0],
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
    page_text: [{ ok: { runs: [RUN] } }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator("#edit-toolbar:visible").waitFor();
  await app.page.locator(".text-layer span").first().waitFor({ state: "attached" });
  return app;
}

async function waitForCall(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

async function screenPoint(app, x, y) {
  const box = await app.page.locator(".page .edit-layer").first().boundingBox();
  return { x: box.x + (x / 595) * box.width, y: box.y + (y / 842) * box.height };
}

async function drag(app, from, to) {
  const a = await screenPoint(app, from.x, from.y);
  const b = await screenPoint(app, to.x, to.y);
  await app.page.mouse.move(a.x, a.y);
  await app.page.mouse.down();
  for (let i = 1; i <= 6; i += 1) await app.page.mouse.move(a.x + ((b.x - a.x) * i) / 6, a.y + ((b.y - a.y) * i) / 6);
  await app.page.mouse.up();
}

const near = (a, b, tolerance = 1.5) => Math.abs(a - b) <= tolerance;

test("Caviarder : une zone tracée hors du texte devient une zone caviardée", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 5, view: view() } }] });
  try {
    await app.page.click("#tool-redact");
    assert.equal(await app.page.locator('[data-tool-options="redact"]').isVisible(), true);
    await drag(app, { x: 100, y: 200 }, { x: 300, y: 300 });
    const [added] = await waitForCall(app, "add_element");
    const { kind, page, areas } = added.element;
    assert.deepEqual([kind, page, areas.length], ["redact", 0, 1]);
    const [area] = areas;
    assert.ok(near(area.top_left.x, 100) && near(area.top_left.y, 200), JSON.stringify(area));
    assert.ok(near(area.width, 200) && near(area.height, 100), JSON.stringify(area));
    assert.equal(await app.page.locator(".draft").count(), 0);
    // La zone ajoutée est choisie : aucune erreur.
    await app.page.locator(".edit-layer .element.selected").waitFor();
    assert.equal(await app.page.locator("#toast.error").isVisible(), false);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Caviarder : glisser sur un mot caviarde le texte sélectionné, à la hauteur du PDF", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 5, view: view() } }] });
  try {
    await app.page.click("#tool-redact");
    await drag(app, { x: RUN.x + 1, y: RUN.y + 7 }, { x: 450, y: RUN.y + 7 });
    const [added] = await waitForCall(app, "add_element");
    const { kind, areas } = added.element;
    assert.equal(kind, "redact");
    assert.equal(areas.length, 1);
    assert.ok(near(areas[0].top_left.x, RUN.x, 2) && near(areas[0].width, RUN.width, 3), JSON.stringify(areas[0]));
    assert.ok(near(areas[0].top_left.y, RUN.y, 0.01) && near(areas[0].height, RUN.height, 0.01), JSON.stringify(areas[0]));
  } finally {
    await app.close();
  }
});

test("Zone caviardée choisie : panneau sans couleur, avec l'explication", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: view() }] });
  try {
    await app.page.locator(".edit-layer .element").first().click();
    await app.page.locator("#prop-fields:visible").waitFor();
    assert.equal(await app.page.textContent("#properties-title"), "Zone caviardée");
    assert.equal(await app.page.locator("#prop-redact-hint").isVisible(), true);
    assert.equal(await app.page.locator("#prop-annot-fields").isVisible(), false);
    // Bogue corrigé : une zone caviardée n'a pas de couleur ; les commandes
    // de style ne doivent pas la lire.
    assert.deepEqual(app.errors, []);
    assert.equal(await app.page.locator("#toast.error").isVisible(), false);
  } finally {
    await app.close();
  }
});

test("Enregistrer un document caviardé demande confirmation, une fois ; Annuler n'écrit rien", async () => {
  const saved = { path: "/docs/contrat.pdf", file_name: "contrat.pdf" };
  const app = await openInEditMode({
    edit_view: [{ ok: view() }, { ok: { ...view(), dirty: false } }],
    save_document: [{ ok: saved }],
  });
  try {
    await app.page.locator("#doc-dirty:visible").waitFor();
    await app.page.keyboard.press("Control+s");
    await app.page.locator("#redact-dialog[open]").waitFor();
    assert.match(await app.page.textContent("#redact-text"), /supprimé de « contrat\.pdf »/);
    // Bogue corrigé : le libellé du bouton rouge restait de la couleur du fond.
    const confirm = await app.page.$eval("#redact-confirm", (b) => [getComputedStyle(b).color, getComputedStyle(b).backgroundColor]);
    assert.notEqual(confirm[0], confirm[1], "libellé lisible");
    await app.page.click("#redact-cancel");
    assert.deepEqual(await app.callsTo("save_document"), []);
    // Confirmé : enregistré ; la fois suivante, plus de question.
    await app.page.keyboard.press("Control+s");
    await app.page.click("#redact-confirm");
    await waitForCall(app, "save_document");
    await app.page.keyboard.press("Control+Shift+s");
    await waitForCall(app, "save_document_as");
    assert.equal(await app.page.locator("#redact-dialog[open]").count(), 0);
  } finally {
    await app.close();
  }
});
