// Traduction (IA-06 à IA-08) : langue proposée par les réglages,
// estimation, traduction posée sur le document (remplacer, bilingue) ou
// gardée en texte, débordements et lignes manquantes signalés ;
// glossaire dans les Réglages. Aucune vraie API.

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const READY = { configured: true, label: "local (localhost)", host: "localhost", has_key: false, local: true };
const ESTIMATE = { requests: 2, input_tokens: 3000, output_tokens: 3200, cost: 0, currency: "EUR", characters: 6000, host: "localhost", needs_confirmation: false, over_cap: false, month_cost: 0 };

async function openDocument(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: READY }],
    ai_translate_estimate: [{ ok: ESTIMATE }],
    ai_translate_start: [{ ok: 6 }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-ai");
  return app;
}

async function waitFor(app, command) {
  await app.page.waitForFunction((name) => window.__calls.some((c) => c.command === name), command);
  return (await app.callsTo(command))[0];
}

test("remplacer le texte : traduction posée, débordements et lignes manquantes signalés", async () => {
  const app = await openDocument();
  const { page } = app;
  try {
    assert.equal(await page.inputValue("#ai-language"), "anglais", "langue des réglages");
    await page.fill("#ai-language", "allemand");
    await page.click("#ai-translate");
    const estimate = await waitFor(app, "ai_translate_estimate");
    assert.deepEqual(estimate, { id: 1, scope: { kind: "document" }, language: "allemand" });
    const start = await waitFor(app, "ai_translate_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, language: "allemand", mode: "replace", consent: true });
    await page.locator(".ai-answer-title", { hasText: "Traduction en allemand" }).waitFor();

    await app.emit("ai-event", { run: 6, kind: "reading", done: 0, total: 2 });
    await page.locator(".ai-status", { hasText: "partie 1 sur 2" }).waitFor();
    await app.emit("ai-done", {
      run: 6,
      Ok: {
        text: "## Page 1\nMietvertrag\nZwischen den Parteien\n",
        cost: 0,
        currency: "EUR",
        model: "local (localhost)",
        fallback_used: false,
        applied: {
          pages: { document: documentView(3), edit: { ...EMPTY_EDIT, revisions: [1, 0, 0], can_undo: true, dirty: true }, selected: [] },
          blocks: 2,
          overflowing: 1,
        },
        missing: 1,
      },
    });
    const answer = page.locator(".ai-answer").last();
    await answer.locator("h3", { hasText: "Page 1" }).waitFor();
    const notes = await answer.locator(".ai-translate-note").allTextContents();
    assert.equal(notes.length, 4, notes.join(" | "));
    assert.match(notes[0], /2 ligne\(s\) traduite\(s\)/);
    assert.match(notes[1], /Enregistrer sous/);
    assert.match(notes[2], /1 ligne\(s\) restent trop longues/);
    assert.match(notes[3], /1 ligne\(s\) non traduite\(s\)/);
    await page.locator("#doc-dirty:not([hidden])").waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("texte seul : le document ne change pas", async () => {
  const app = await openDocument();
  const { page } = app;
  try {
    await page.selectOption("#ai-translate-mode", "text");
    await page.click('[data-scope="page"]');
    await page.click("#ai-translate");
    const start = await waitFor(app, "ai_translate_start");
    assert.equal(start.mode, "text");
    assert.deepEqual(start.scope, { kind: "pages", pages: [0] });
    await app.emit("ai-done", {
      run: 6,
      Ok: { text: "## Page 1\nLease agreement\n", cost: 0, currency: "EUR", model: "m", fallback_used: false, applied: null, missing: 0 },
    });
    const answer = page.locator(".ai-answer").last();
    await answer.locator("p", { hasText: "Lease agreement" }).waitFor();
    assert.equal(await answer.locator(".ai-translate-note").count(), 0);
    assert.equal(await page.isHidden("#doc-dirty"), true);
  } finally {
    await app.close();
  }
});

test("Réglages : glossaire et corps minimal enregistrés", async () => {
  const app = await launch({ save_settings: [{ ok: DEFAULT_SETTINGS }] });
  const { page } = app;
  try {
    await page.click("#menu-button");
    await page.click("#menu-settings");
    await page.click('.settings-nav [data-section="ai"]');
    await page.fill("#set-tr-glossary", "bail = lease\nPdfEditor");
    await page.fill("#set-tr-min", "7");
    await page.click("#settings-save");
    await page.waitForFunction(() => window.__calls.some((c) => c.command === "save_settings"));
    const [saved] = await app.callsTo("save_settings");
    assert.deepEqual(saved.settings.ai.translation, { language: "anglais", min_size: 7, glossary: "bail = lease\nPdfEditor" });
  } finally {
    await app.close();
  }
});
