// Recettes de l'assistant (IA-16) : rubrique des Réglages (intégrées en
// lecture seule, duplication, enregistrement) et choix dans le mode IA,
// qui part avec la demande. Aucune vraie API.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const READY = { configured: true, label: "local (localhost)", host: "localhost", has_key: false, local: true };
const ESTIMATE = { requests: 1, input_tokens: 900, output_tokens: 1024, cost: 0, currency: "EUR", characters: 2000, host: "localhost", needs_confirmation: false, over_cap: false, month_cost: 0 };
const RGPD = {
  id: "integree-rgpd",
  builtin: true,
  recipe: { name: "Anonymiser selon le RGPD", description: "Données personnelles", instruction: "Caviarde les noms.", actions: ["redact"], scope: "document" },
};
const MINE = {
  id: "dates",
  builtin: false,
  recipe: { name: "Dates", description: "", instruction: "Surligne les dates.", actions: ["highlight"], scope: "page" },
};

test("Réglages : recette intégrée en lecture seule, dupliquée puis enregistrée", async () => {
  const copy = { id: "anonymiser-copie", builtin: false, recipe: { ...RGPD.recipe, name: "Anonymiser selon le RGPD (copie)" } };
  const app = await launch({
    recipes_list: [{ ok: [RGPD, MINE] }, { ok: [RGPD, copy, MINE] }],
    recipe_save: [{ ok: copy }],
  });
  const { page } = app;
  try {
    await page.click("#menu-button");
    await page.click("#menu-settings");
    await page.click('.settings-nav [data-section="recipes"]');
    assert.equal(await page.inputValue("#recipe-name"), "Anonymiser selon le RGPD");
    assert.equal(await page.isDisabled("#recipe-instruction"), true);
    assert.equal(await page.isVisible("#recipe-builtin"), true);
    assert.equal(await page.isVisible("#recipe-save"), false);
    assert.equal(await page.isChecked('.recipe-actions input[value="redact"]'), true);

    await page.click("#recipe-duplicate");
    assert.equal(await page.inputValue("#recipe-name"), "Anonymiser selon le RGPD (copie)");
    assert.equal(await page.isDisabled("#recipe-instruction"), false);
    // Sans action permise : refusé avant tout envoi.
    await page.uncheck('.recipe-actions input[value="redact"]');
    await page.click("#recipe-save");
    await page.locator("#recipe-error", { hasText: "au moins une action" }).waitFor();
    await page.check('.recipe-actions input[value="redact"]');
    await page.check('.recipe-actions input[value="add_note"]');
    await page.click("#recipe-save");
    await page.waitForFunction(() => window.__calls.some((c) => c.command === "recipe_save"));
    const [saved] = await app.callsTo("recipe_save");
    assert.equal(saved.id, null);
    assert.deepEqual(saved.recipe.actions, ["redact", "add_note"]);
    await page.locator("#toast", { hasText: "enregistrée" }).waitFor();
    await page.waitForFunction(() => document.getElementById("recipe-pick").value === "anonymiser-copie");
    assert.equal(await page.isVisible("#recipe-delete"), true);
  } finally {
    await app.close();
  }
});

test("mode IA : la recette choisie part avec la demande, facultative", async () => {
  const app = await launch({
    recipes_list: [{ ok: [RGPD, MINE] }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: READY }],
    ai_estimate: [{ ok: ESTIMATE }],
    ai_start: [{ ok: 3 }],
  });
  const { page } = app;
  try {
    await page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    await page.click("#mode-ai");
    assert.equal(await page.isVisible("#ai-recipe"), false, "seulement en « Action »");
    await page.click('[data-ask="assist"]');
    await page.selectOption("#ai-recipe", "dates");
    // Portée de la recette proposée : la page affichée.
    assert.equal(await page.locator('[data-scope="page"]').getAttribute("aria-checked"), "true");
    await page.selectOption("#ai-recipe", "integree-rgpd");
    assert.equal(await page.locator('[data-scope="document"]').getAttribute("aria-checked"), "true");
    assert.match(await page.getAttribute("#ai-question", "placeholder"), /Précisions/);

    // Demande vide : la recette suffit.
    await page.click("#ai-send");
    await page.waitForFunction(() => window.__calls.some((c) => c.command === "ai_start"));
    const [estimate] = await app.callsTo("ai_estimate");
    assert.equal(estimate.recipe, "integree-rgpd");
    const [start] = await app.callsTo("ai_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, task: { kind: "assist", question: "" }, recipe: "integree-rgpd", consent: true });
    await page.locator(".ai-answer-title", { hasText: "Anonymiser selon le RGPD" }).waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
