// Palette de commandes (Ctrl+K) et « Comment faire ? » (F1), tirées du
// catalogue des actions (V-08) : exécuter une action, montrer son outil.

import assert from "node:assert/strict";
import { test } from "node:test";

import { ACTION_CATALOG, documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };

async function openDocument() {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: { configured: false, label: "", host: "", has_key: false, local: false } }],
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

/** Ouvre la palette au clavier et tape `query`. */
async function search(page, key, query) {
  await page.locator("body").press(key);
  await page.locator("#palette[open]").waitFor();
  await page.fill("#palette-input", query);
}

test("chaque action du catalogue a son outil dans l'interface", async () => {
  const app = await openDocument();
  try {
    const missing = await app.page.evaluate(
      (actions) => actions.filter((a) => !document.querySelector(a.tool)).map((a) => `${a.id} (${a.tool})`),
      ACTION_CATALOG,
    );
    assert.deepEqual(missing, []);
  } finally {
    await app.close();
  }
});

test("sans document : la palette ouvre les Réglages et explique ce qui attend un document", async () => {
  const app = await launch();
  const { page } = app;
  try {
    await search(page, "Control+k", "pivoter");
    const rotate = page.locator('.palette-item[data-action="rotate_pages"]');
    assert.equal(await rotate.getAttribute("aria-disabled"), "true");
    assert.match(await rotate.textContent(), /Ouvrez d'abord un document/);
    await page.keyboard.press("Enter");
    await page.locator("#toast", { hasText: "Ouvrez d'abord un document" }).waitFor();
    assert.equal(await page.locator("#palette[open]").count(), 1, "la palette reste ouverte");

    await page.fill("#palette-input", "raccourcis");
    assert.equal(await page.locator(".palette-item").first().getAttribute("data-action"), "settings");
    await page.keyboard.press("Enter");
    await page.locator("#settings-view:not([hidden])").waitFor();
    assert.equal(await page.locator("#palette[open]").count(), 0);
  } finally {
    await app.close();
  }
});

test("Ctrl+K : l'action choisie s'exécute, dans son mode", async () => {
  const app = await openDocument();
  const { page } = app;
  try {
    await search(page, "Control+k", "caviar");
    // Raccourci affiché à côté des actions qui en ont un.
    await page.fill("#palette-input", "zoom avant");
    assert.equal(await page.locator('.palette-item[data-action="zoom_in"] kbd').textContent(), "Ctrl++");
    const zoom = await page.textContent("#zoom-value");
    await page.keyboard.press("Enter");
    await page.waitForFunction((before) => document.querySelector("#zoom-value").textContent !== before, zoom);

    // Flèches puis Entrée : Caviarder passe en mode Éditer avec l'outil choisi.
    await search(page, "Control+k", "masquer");
    const ids = await page.locator(".palette-item").evaluateAll((items) => items.map((i) => i.dataset.action));
    for (let i = 0; i < ids.indexOf("redact"); i++) await page.keyboard.press("ArrowDown");
    assert.equal(await page.locator('.palette-item[aria-selected="true"]').getAttribute("data-action"), "redact");
    await page.keyboard.press("Enter");
    await page.locator('#mode-edit[aria-current="page"]').waitFor();
    assert.equal(await page.locator("#tool-redact").getAttribute("aria-pressed"), "true");
  } finally {
    await app.close();
  }
});

test("Comment faire ? (F1) : l'outil est montré dans la barre, sans rien exécuter", async () => {
  const app = await openDocument();
  const { page } = app;
  try {
    await search(page, "F1", "comment tourner une page ?");
    assert.equal(await page.textContent("#palette-title"), "Comment faire ?");
    const first = page.locator(".palette-item").first();
    assert.equal(await first.getAttribute("data-action"), "rotate_pages");
    assert.match(await first.textContent(), /quart de tour/);
    const before = (await app.calls()).length;
    await page.keyboard.press("Enter");

    await page.locator('#mode-pages[aria-current="page"]').waitFor();
    await page.locator("#pages-rotate-right.spotlight").waitFor();
    assert.equal(await page.evaluate(() => document.activeElement?.id), "pages-rotate-right");
    await page.locator("#toast", { hasText: "Pivoter des pages" }).waitFor();
    const after = await app.calls();
    assert.ok(!after.slice(before).some((c) => /rotate/.test(c.command)), "rien n'a pivoté");

    // Depuis le menu, et Maj+Entrée dans la palette : montrer aussi.
    await page.click("#menu-button");
    await page.click("#menu-palette");
    await page.fill("#palette-input", "propriétés");
    await page.keyboard.press("Shift+Enter");
    await page.locator("#main-menu:not([hidden]) #menu-properties.spotlight").waitFor();
  } finally {
    await app.close();
  }
});
