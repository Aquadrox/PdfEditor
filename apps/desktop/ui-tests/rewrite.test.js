// Modification par consigne (IA-09) et anonymisation assistée (IA-10) :
// lignes proposées en différences, acceptées ligne par ligne puis posées ;
// « Anonymiser » lance l'assistant avec la recette RGPD. Aucune vraie API.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const READY = { configured: true, label: "local (localhost)", host: "localhost", has_key: false, local: true };
const ESTIMATE = { requests: 1, input_tokens: 900, output_tokens: 600, cost: 0, currency: "EUR", characters: 1500, host: "localhost", needs_confirmation: false, over_cap: false, month_cost: 0 };
const RGPD = {
  id: "integree-rgpd",
  builtin: true,
  recipe: { name: "Anonymiser selon le RGPD", description: "", instruction: "Caviarde les noms.", actions: ["redact"], scope: "document" },
};
const PROPOSALS = [
  { page: 0, objects: [0, 1], bounds: { x: 72, y: 62, width: 218, height: 25 }, before: "Salut à vous", after: "Bonjour à vous" },
  { page: 1, objects: [3], bounds: { x: 72, y: 100, width: 200, height: 14 }, before: "Le contrat", after: "Le contrat signé" },
];

async function openDocument(commands = {}) {
  const app = await launch({
    recipes_list: [{ ok: [RGPD] }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: READY }],
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

test("modifier : différences ligne par ligne, seules les lignes cochées sont posées", async () => {
  const app = await openDocument({
    ai_rewrite_estimate: [{ ok: ESTIMATE }],
    ai_rewrite_start: [{ ok: 8 }],
    ai_rewrite_apply: [{ ok: { pages: { document: documentView(3), edit: { ...EMPTY_EDIT, revisions: [1, 0, 0], can_undo: true, dirty: true }, selected: [] }, blocks: 1, overflowing: 0 } }],
  });
  const { page } = app;
  try {
    await page.click('[data-ask="rewrite"]');
    assert.match(await page.getAttribute("#ai-question", "placeholder"), /corrige les fautes/);
    await page.fill("#ai-question", "plus formel");
    await page.press("#ai-question", "Enter");
    const start = await waitFor(app, "ai_rewrite_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, instruction: "plus formel", consent: true });
    await app.emit("ai-done", { run: 8, Ok: { proposals: PROPOSALS, cost: 0, currency: "EUR", model: "local", fallback_used: false } });

    const answer = page.locator(".ai-answer").last();
    await answer.locator(".ai-proposals").waitFor();
    assert.equal(await answer.locator(".ai-diff del").first().textContent(), "Salut");
    assert.equal(await answer.locator(".ai-diff ins").first().textContent(), "Bonjour");
    assert.equal(await page.locator(".ai-preview.rewrite").count(), 2, "un cadre par ligne cochée");
    assert.equal(await answer.locator(".ai-apply").textContent(), "Appliquer (2)");
    assert.equal((await app.callsTo("ai_rewrite_apply")).length, 0, "rien de posé avant l'accord");

    await answer.locator('[data-rewrite="1"]').uncheck();
    assert.equal(await answer.locator(".ai-apply").textContent(), "Appliquer (1)");
    assert.equal(await page.locator(".page").nth(1).locator(".ai-preview.rewrite").count(), 0, "ligne décochée : plus de cadre");
    await answer.locator(".ai-apply").click();
    const applied = await waitFor(app, "ai_rewrite_apply");
    assert.deepEqual(applied, { id: 1, accepted: [{ page: 0, objects: [0, 1], before: "Salut à vous", text: "Bonjour à vous" }] });
    await answer.locator(".ai-proposal-actions", { hasText: "1 ligne(s) modifiée(s)" }).waitFor();
    await page.locator("#doc-dirty:not([hidden])").waitFor();
    assert.equal(await page.locator(".ai-preview").count(), 0);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("anonymiser : l'assistant part avec la recette RGPD", async () => {
  const app = await openDocument({ ai_estimate: [{ ok: ESTIMATE }], ai_start: [{ ok: 9 }] });
  const { page } = app;
  try {
    await page.click("#ai-anonymize");
    const start = await waitFor(app, "ai_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, task: { kind: "assist", question: "" }, recipe: "integree-rgpd", consent: true });
    assert.equal(await page.locator('[data-ask="assist"]').getAttribute("aria-checked"), "true");
    await page.locator(".ai-answer-title", { hasText: "Anonymiser selon le RGPD" }).waitFor();
  } finally {
    await app.close();
  }
});
