// Assistant d'actions (IA-15) : demande en langage courant, propositions
// vérifiées par pde-app montrées en aperçu, appliquées seulement sur
// accord ; repli sur « Comment faire ? ». Aucune vraie API.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const READY = { configured: true, label: "deepseek-flash (api.deepseek.com)", host: "api.deepseek.com", has_key: true, local: false };
const ESTIMATE = { requests: 1, input_tokens: 2000, output_tokens: 1024, cost: 0.001, currency: "USD", characters: 5400, host: "api.deepseek.com", needs_confirmation: false, over_cap: false, month_cost: 0.2 };

const REDACT = {
  proposal: { action: "redact", text: "Jean Dupont", pages: [] },
  summary: "Caviarder « Jean Dupont »",
  pages: [0],
  previews: [{ page: 0, area: { x: 72, y: 100, width: 80, height: 12 } }],
  redaction: true,
};
const ROTATE = {
  proposal: { action: "rotate_pages", pages: [2], direction: "right" },
  summary: "Pivoter (p. 2) à droite",
  pages: [1],
  previews: [],
  redaction: false,
};

async function openDocument(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: READY }],
    ai_estimate: [{ ok: ESTIMATE }],
    ai_start: [{ ok: 4 }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

/** Mode IA, champ en « Action », demande envoyée ; renvoie la réponse en cours. */
async function request(app, text) {
  const { page } = app;
  await page.click("#mode-ai");
  await page.click('[data-ask="assist"]');
  assert.equal(await page.textContent("#ai-send"), "Proposer");
  await page.fill("#ai-question", text);
  await page.press("#ai-question", "Enter");
  await page.waitForFunction(() => window.__calls.some((c) => c.command === "ai_start"));
  const [start] = await app.callsTo("ai_start");
  assert.deepEqual(start.task, { kind: "assist", question: text });
  return page.locator(".ai-answer.assist").last();
}

test("propositions en aperçu, appliquées seulement si cochées et acceptées", async () => {
  const app = await openDocument({
    assistant_apply: [{ ok: { document: documentView(3), edit: { ...EMPTY_EDIT, revisions: [1, 0, 0], can_undo: true, dirty: true }, selected: [] } }],
  });
  const { page } = app;
  try {
    const answer = await request(app, "caviarde le nom du client et tourne la page 2");
    await app.emit("ai-event", { run: 4, kind: "delta", text: "Je propose de caviarder le nom et de tourner la page 2." });
    await app.emit("ai-done", {
      run: 4,
      Ok: {
        text: "Je propose de caviarder le nom et de tourner la page 2.",
        cost: 0.0004,
        currency: "USD",
        model: "deepseek-flash (api.deepseek.com)",
        fallback_used: false,
        proposals: [REDACT, ROTATE],
        rejected: ["« Dupont SA » introuvable dans la portée choisie"],
      },
    });
    await answer.locator(".ai-proposals").waitFor();
    assert.equal(await answer.locator(".ai-proposal-list li").count(), 2);
    assert.match(await answer.locator(".ai-rejected summary").textContent(), /1 proposition\(s\) écartée\(s\)/);
    assert.match(await answer.locator(".ai-redact-note").textContent(), /Masquer .* n'est pas proposé/);
    // Aperçu : un cadre rouge (caviardage) sur la page 1, rien n'est encore appliqué.
    assert.equal(await page.locator(".page").nth(0).locator(".ai-preview.redact").count(), 1);
    assert.equal((await app.callsTo("assistant_apply")).length, 0);

    // Décocher la rotation, puis appliquer : seule la proposition cochée part.
    await answer.locator('[data-proposal="1"]').uncheck();
    await answer.locator(".ai-apply").click();
    await page.waitForFunction(() => window.__calls.some((c) => c.command === "assistant_apply"));
    const [apply] = await app.callsTo("assistant_apply");
    assert.deepEqual(apply, { id: 1, scope: { kind: "document" }, proposals: [REDACT.proposal] });
    await answer.locator(".ai-proposal-actions", { hasText: "1 action(s) appliquée(s)" }).waitFor();
    assert.equal(await page.locator(".ai-preview").count(), 0);
    // L'édition reprend l'état rendu : « modifié », Annuler possible.
    await page.locator("#doc-dirty:not([hidden])").waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Refuser : rien n'est appliqué ; sans proposition, « Comment faire ? » prend le relais", async () => {
  const app = await openDocument({ ai_start: [{ ok: 4 }, { ok: 5 }] });
  const { page } = app;
  try {
    const first = await request(app, "caviarde le nom");
    await app.emit("ai-done", {
      run: 4,
      Ok: { text: "Voici.", cost: 0, currency: "USD", model: "m", fallback_used: false, proposals: [REDACT], rejected: [] },
    });
    await first.locator(".ai-apply").waitFor();
    await first.locator("button", { hasText: "Refuser" }).click();
    await first.locator(".ai-proposal-actions", { hasText: "rien n'a changé" }).waitFor();
    assert.equal(await page.locator(".ai-preview").count(), 0);
    assert.equal((await app.callsTo("assistant_apply")).length, 0);

    // Modèle sans outils : une explication, et l'aide sans IA à portée de clic.
    await page.fill("#ai-question", "tourner une page");
    await page.press("#ai-question", "Enter");
    await page.waitForFunction(() => window.__calls.filter((c) => c.command === "ai_start").length === 2);
    await app.emit("ai-done", {
      run: 5,
      Ok: { text: "Utilisez le mode Pages.", cost: 0, currency: "USD", model: "m", fallback_used: false, proposals: [], rejected: [] },
    });
    const second = page.locator(".ai-answer.assist").last();
    await second.locator("button", { hasText: "Comment faire ?" }).click();
    await page.locator("#palette[open]").waitFor();
    assert.equal(await page.inputValue("#palette-input"), "tourner une page");
    assert.equal(await page.textContent("#palette-title"), "Comment faire ?");
    assert.equal(await page.locator(".palette-item").first().getAttribute("data-action"), "rotate_pages");
  } finally {
    await app.close();
  }
});

test("« Renomme X en Y » : l'assistant confie la modification du texte à « Modifier »", async () => {
  // Bogue : l'assistant n'avait aucun outil pour changer le texte.
  const EDIT = {
    proposal: { action: "edit_text", instruction: "Renomme Kaelen Vane en Aquadrox", pages: [] },
    summary: "Modifier le texte : « Renomme Kaelen Vane en Aquadrox »",
    pages: [0, 1, 2],
    previews: [],
    redaction: false,
  };
  const app = await openDocument({
    ai_rewrite_estimate: [{ ok: ESTIMATE }],
    ai_rewrite_start: [{ ok: 9 }],
  });
  const { page } = app;
  try {
    const answer = await request(app, "Renomme Kaelen Vane en Aquadrox");
    await app.emit("ai-done", {
      run: 4,
      Ok: { text: "Je remplace le nom.", cost: 0, currency: "USD", model: "m", fallback_used: false, proposals: [EDIT], rejected: [] },
    });
    await answer.locator(".ai-handoff").waitFor();
    assert.equal(await answer.locator(".ai-apply").count(), 0, "rien à appliquer dans la réponse de l'assistant");
    await page.waitForFunction(() => window.__calls.some((c) => c.command === "ai_rewrite_start"));
    const [start] = await app.callsTo("ai_rewrite_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, instruction: "Renomme Kaelen Vane en Aquadrox", consent: true });
    const proposals = [{ page: 0, objects: [2], bounds: { x: 72, y: 80, width: 200, height: 14 }, before: "Kaelen Vane est là", after: "Aquadrox est là" }];
    await app.emit("ai-done", { run: 9, Ok: { proposals, cost: 0, currency: "USD", model: "m", fallback_used: false } });
    const rewrite = page.locator(".ai-answer.rewrite").last();
    await rewrite.locator(".ai-apply", { hasText: "Appliquer (1)" }).waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
