// Notes (E-04) et réglages Édition (R-03).

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };
const YELLOW = { r: 0xff, g: 0xd5, b: 0x4a };
const NOTE = { kind: "note", page: 0, top_left: { x: 290, y: 390 }, text: "", author: "Camille Martin", color: YELLOW };
const noteView = (note) => ({
  elements: [{ id: 3, element: note, bounds: { ...note.top_left, width: 20, height: 20 } }],
  revisions: [1, 0, 0],
  can_undo: true,
  can_redo: false,
  dirty: true,
});

function settingsWith(change) {
  const settings = structuredClone(DEFAULT_SETTINGS);
  change(settings);
  return settings;
}

async function openInEditMode(settings, commands = {}) {
  const app = await launch({
    get_settings: [{ ok: { settings, warnings: [] } }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator("#edit-toolbar:visible").waitFor();
  return app;
}

async function waitForCall(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

async function clickPage(app, x, y) {
  const box = await app.page.locator(".page .edit-layer").first().boundingBox();
  await app.page.mouse.click(box.x + (x / 595) * box.width, box.y + (y / 842) * box.height);
}

test("outil Note : la note porte l'auteur des réglages, son commentaire s'écrit aussitôt", async () => {
  const settings = settingsWith((s) => (s.edit.author = "Camille Martin"));
  const app = await openInEditMode(settings, {
    add_element: [{ ok: { id: 3, view: noteView(NOTE) } }],
    update_element: [{ ok: noteView({ ...NOTE, text: "À vérifier avant signature" }) }],
  });
  try {
    await app.page.click("#tool-note");
    await clickPage(app, 300, 400);
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.kind, e.page, e.text, e.author], ["note", 0, "", "Camille Martin"]);
    assert.ok(Math.abs(e.top_left.x - 290) < 2 && Math.abs(e.top_left.y - 390) < 2, JSON.stringify(e.top_left));
    assert.deepEqual(e.color, YELLOW);

    // Panneau Note ouvert, commentaire prêt à écrire.
    await app.page.locator("#prop-note-fields:visible").waitFor();
    assert.equal(await app.page.textContent("#properties-title"), "Note");
    assert.equal(await app.page.evaluate(() => document.activeElement.id), "prop-note");
    assert.equal(await app.page.textContent("#prop-note-author"), "Auteur : Camille Martin");
    await app.page.keyboard.type("À vérifier avant signature");
    await app.page.press("#prop-note", "Tab");
    const [update] = await waitForCall(app, "update_element");
    assert.equal(update.element.text, "À vérifier avant signature");
    assert.equal(update.element.author, "Camille Martin");
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("réglages Édition : police, taille et couleur des nouvelles zones de texte", async () => {
  const settings = settingsWith((s) => {
    s.edit.font = "serif";
    s.edit.font_size = 14;
    s.edit.text_color = "#2747b8";
  });
  const app = await openInEditMode(settings, { add_element: [{ ok: { id: 1, view: EMPTY } }] });
  try {
    assert.equal(await app.page.inputValue('#edit-toolbar [data-style="font"]'), "serif");
    assert.equal(await app.page.inputValue('#edit-toolbar [data-style="size"]'), "14");
    await clickPage(app, 100, 200);
    await app.page.keyboard.type("Bonjour");
    await app.page.keyboard.press("Control+Enter");
    const [added] = await waitForCall(app, "add_element");
    const style = added.element.style;
    assert.deepEqual([style.font, style.size], ["serif", 14]);
    assert.deepEqual(style.color, { r: 0x27, g: 0x47, b: 0xb8 });
  } finally {
    await app.close();
  }
});

test("réglages Édition : couleur de surlignage reprise par l'outil Surligner", async () => {
  const settings = settingsWith((s) => (s.edit.highlight_color = "#a0e0ff"));
  const app = await openInEditMode(settings);
  try {
    await app.page.click("#tool-markup");
    const pressed = app.page.locator('#edit-toolbar [data-tool-options="markup"] [data-annot="color"][aria-pressed="true"]');
    assert.equal(await pressed.count(), 0, "aucune pastille : couleur personnalisée");
    assert.equal(await app.page.inputValue('#edit-toolbar [data-tool-options="markup"] input[data-annot="color"]'), "#a0e0ff");
  } finally {
    await app.close();
  }
});

test("page Réglages, rubrique Édition : enregistrée avec le reste", async () => {
  const saved = settingsWith((s) => (s.edit.author = "Camille Martin"));
  const app = await launch({ save_settings: [{ ok: saved }] });
  try {
    await app.page.click("#menu-button");
    await app.page.click("#menu-settings");
    await app.page.click(".settings-nav [data-section=edit]");
    assert.equal(await app.page.inputValue("#set-font"), "sans");
    assert.equal(await app.page.inputValue("#set-font-size"), "11");
    assert.equal(await app.page.inputValue("#set-highlight-color"), "#ffd54a");
    await app.page.fill("#set-author", "Camille Martin");
    await app.page.selectOption("#set-font", "mono");
    await app.page.click("#settings-save");
    const [save] = await waitForCall(app, "save_settings");
    assert.equal(save.settings.edit.author, "Camille Martin");
    assert.equal(save.settings.edit.font, "mono");
  } finally {
    await app.close();
  }
});
