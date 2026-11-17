// Texte du document (E-05) : avec l'outil « Texte du document », chaque
// ligne se propose ; un clic l'ouvre à la saisie, à sa place. Rien n'est
// créé si le texte ne change pas ; la modification se reprend, se vide
// (suppression) et signale une police remplacée.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0, 0, 0], layout: 0, can_undo: false, can_redo: false, dirty: false };
const STYLE = { font: "sans", size: 24, bold: false, italic: false, color: { r: 0, g: 0, b: 0 }, align: "left", line_height: 1.25 };
const BLOCK = { objects: [0, 1], bounds: { x: 72, y: 62, width: 218, height: 25 }, text: "Bonjour PdfEditor", font_name: "Helvetica", style: STYLE };
const ORIGINAL = { top_left: { x: 72, y: 62 }, width: 218, height: 25 };

function view(text, { replaced = false, id = 5 } = {}) {
  const element = { kind: "original_text", page: 0, objects: [0, 1], original: ORIGINAL, before: BLOCK.text, text, style: STYLE };
  return {
    elements: [{ id, element, bounds: { x: 72, y: 62, width: 218, height: 25 }, missing_chars: "", replaced_font: replaced }],
    revisions: [1, 0, 0],
    layout: 0,
    can_undo: true,
    can_redo: false,
    dirty: true,
  };
}

async function openWithTool(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    page_images: [{ ok: [] }],
    page_text_blocks: [{ ok: [BLOCK] }],
    ...commands,
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    await app.page.click("#mode-edit");
    await app.page.locator("#edit-toolbar:visible").waitFor();
    await app.page.click("#tool-retext");
    await block(app).waitFor();
  } catch (error) {
    await app.close();
    throw error;
  }
  return app;
}

const block = (app) => app.page.locator(".page").first().locator('.element.text-block[data-id="otext:0:0"]');
const editor = (app) => app.page.locator(".text-editor.opaque");

async function waitForCall(app, command) {
  await app.page.waitForFunction((name) => window.__calls.some((c) => c.command === name), command);
  return app.callsTo(command);
}

test("Texte du document : lignes proposées seulement avec l'outil", async () => {
  const app = await openWithTool();
  try {
    assert.equal(await app.page.getAttribute("#tool-retext", "aria-pressed"), "true");
    await app.page.click("#tool-text");
    await block(app).waitFor({ state: "detached" });
    await app.page.click("#tool-retext");
    await block(app).waitFor();
    // Une seule lecture des lignes de la page.
    assert.equal((await app.callsTo("page_text_blocks")).filter((c) => c.page === 0).length, 1);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Texte du document : saisi à sa place, validé par Entrée, en une opération", async () => {
  const app = await openWithTool({ add_element: [{ ok: { id: 5, view: view("Bonsoir PdfEditor") } }] });
  try {
    await block(app).click();
    await editor(app).waitFor();
    assert.equal(await editor(app).inputValue(), "Bonjour PdfEditor");
    assert.equal(await editor(app).getAttribute("rows"), "1");
    // La ligne proposée laisse la place à la saisie.
    assert.equal(await block(app).count(), 0);
    await editor(app).fill("Bonsoir PdfEditor");
    await editor(app).press("Enter");
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.kind, e.page, e.objects, e.text, e.before], ["original_text", 0, [0, 1], "Bonsoir PdfEditor", "Bonjour PdfEditor"]);
    assert.deepEqual(e.original, ORIGINAL);
    assert.equal(e.style.size, 24);
    // La modification remplace la ligne proposée ; le style des nouvelles zones ne change pas.
    await app.page.locator('.element[data-id="5"]').waitFor();
    assert.equal(await block(app).count(), 0);
    await app.page.click("#tool-text");
    assert.equal(await app.page.inputValue("#edit-toolbar .size-input"), "11");
  } finally {
    await app.close();
  }
});

test("Texte du document inchangé ou annulé : rien n'est créé", async () => {
  const app = await openWithTool();
  try {
    await block(app).click();
    await editor(app).press("Enter");
    await block(app).waitFor();
    await block(app).click();
    await editor(app).fill("Autre chose");
    await editor(app).press("Escape");
    await block(app).waitFor();
    assert.deepEqual(await app.callsTo("add_element"), []);
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), false);
  } finally {
    await app.close();
  }
});

test("Texte du document modifié : reprise au double-clic, police remplacée signalée, Suppr le vide", async () => {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: view("Привет", { replaced: true }) }],
    page_images: [{ ok: [] }],
    update_element: [{ ok: view("Привет мир", { replaced: true }) }, { ok: view("") }],
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    await app.page.click("#mode-edit");
    const edited = app.page.locator('.element[data-id="5"]');
    await edited.click();
    assert.equal(await app.page.textContent("#properties-title"), "Texte du document");
    assert.match(await app.page.textContent("#prop-original-font"), /ne contient pas ces caractères.*Helvetica/);
    assert.equal(await app.page.locator("#prop-geometry").isVisible(), false);
    assert.equal(await app.page.locator("#prop-duplicate").isVisible(), false);
    // Ni poignée ni déplacement : il garde sa place et sa taille.
    assert.equal(await edited.locator(".handle").count(), 0);
    // Glisser ne déplace pas le texte d'origine.
    const box = await edited.boundingBox();
    await app.page.mouse.move(box.x + 10, box.y + 5);
    await app.page.mouse.down();
    await app.page.mouse.move(box.x + 80, box.y + 60, { steps: 5 });
    await app.page.mouse.up();
    await edited.dblclick();
    await editor(app).fill("Привет\nмир");
    await editor(app).blur();
    const [first] = await waitForCall(app, "update_element");
    assert.equal(first.element.text, "Привет мир");
    await edited.click();
    await app.page.keyboard.press("Delete");
    await app.page.waitForFunction(() => window.__calls.filter((c) => c.command === "update_element").length === 2);
    const calls = await app.callsTo("update_element");
    assert.equal(calls.length, 2);
    assert.deepEqual([calls[1].elementId, calls[1].element.text], [5, ""]);
    // Vidé : il n'est plus montré.
    await edited.waitFor({ state: "detached" });
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
