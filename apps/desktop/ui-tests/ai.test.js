// Mode IA (IA-01 à IA-05, IA-12) : portée, estimation et confirmation,
// réponse en flux avec citations de pages, plafond, annulation ; rubrique
// IA des Réglages. Aucune vraie API : les commandes Rust sont simulées.

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const READY = { configured: true, label: "deepseek-flash (api.deepseek.com)", host: "api.deepseek.com", has_key: true, local: false };
const ESTIMATE = { requests: 1, input_tokens: 2000, output_tokens: 2048, cost: 0.0031, currency: "USD", characters: 5400, host: "api.deepseek.com", needs_confirmation: true, over_cap: false, month_cost: 0.2 };

async function openDocument(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ai_status: [{ ok: READY }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

async function waitForCalls(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

test("IA non configurée : le panneau mène aux Réglages", async () => {
  const app = await openDocument({ ai_status: [{ ok: { configured: false, label: "", host: "", has_key: false, local: false } }] });
  const { page } = app;
  try {
    await page.click("#mode-ai");
    await page.locator("#ai-unconfigured:not([hidden])").waitFor();
    assert.equal(await page.locator('[data-task="summary"]').isDisabled(), true);
    await page.click("#ai-open-settings");
    await page.locator("#settings-view:not([hidden])").waitFor();
    assert.equal(await page.locator('.settings-section[data-section="ai"]').isVisible(), true);
  } finally {
    await app.close();
  }
});

test("Résumer le document : confirmation, réponse en flux, citations vérifiées", async () => {
  const app = await openDocument({ ai_estimate: [{ ok: ESTIMATE }], ai_start: [{ ok: 7 }] });
  const { page } = app;
  try {
    await page.click("#mode-ai");
    await page.locator("#ai-provider", { hasText: "deepseek-flash" }).waitFor();
    assert.equal(await page.locator('[data-scope="document"]').getAttribute("aria-checked"), "true");
    assert.equal(await page.locator('[data-scope="selection"]').isDisabled(), true);
    await page.click('[data-task="summary"]');

    // Ce qui part, et où, avant l'envoi.
    await page.locator("#ai-confirm-dialog[open]").waitFor();
    const text = await page.locator("#ai-confirm-text").textContent();
    assert.match(text, /Tout le document \(3 pages\) va partir chez api\.deepseek\.com/);
    assert.match(await page.locator("#ai-confirm-cost").textContent(), /1 requête\(s\), coût estimé au plus 0\.0031 USD/);
    await page.click("#ai-confirm-send");

    const [start] = await waitForCalls(app, "ai_start");
    assert.deepEqual(start, { id: 1, scope: { kind: "document" }, task: { kind: "summary" }, consent: true });
    await page.locator(".ai-answer.running").waitFor();

    await app.emit("ai-event", { run: 7, kind: "delta", text: "## Résumé\nUn contrat de **deux ans** [p. 2]" });
    await app.emit("ai-event", { run: 7, kind: "delta", text: " et une annexe [p. 9].\n- Prix fixe" });
    await page.locator(".ai-answer h3", { hasText: "Résumé" }).waitFor();
    await app.emit("ai-done", {
      run: 7,
      Ok: { text: "## Résumé\nUn contrat de **deux ans** [p. 2] et une annexe [p. 9].\n- Prix fixe", cost: 0.0012, currency: "USD", model: "deepseek-flash (api.deepseek.com)", fallback_used: false },
    });
    await page.locator(".ai-answer:not(.running)").waitFor();
    const answer = page.locator(".ai-answer").first();
    assert.equal(await answer.locator("strong", { hasText: "deux ans" }).count(), 1);
    assert.equal(await answer.locator("li", { hasText: "Prix fixe" }).count(), 1);
    // [p. 9] n'existe pas dans un document de 3 pages : pas de lien.
    assert.equal(await answer.locator("span.ai-cite.invalid", { hasText: "[p. 9]" }).count(), 1);
    await answer.locator("button.ai-cite", { hasText: "[p. 2]" }).click();
    await page.waitForFunction(() => document.getElementById("page-input").value === "2");
    assert.match(await answer.locator(".ai-status").textContent(), /deepseek-flash.*0\.0012 USD/);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Question sur des pages choisies ; plafond mensuel ; annulation", async () => {
  const app = await openDocument({
    ai_estimate: [{ ok: { ...ESTIMATE, over_cap: true } }, { ok: { ...ESTIMATE, needs_confirmation: false } }],
    ai_start: [{ ok: 9 }],
  });
  const { page } = app;
  try {
    await page.click("#mode-ai");
    await page.click('[data-scope="pages"]');
    await page.fill("#ai-pages", "2-9");
    assert.match(await page.locator("#ai-scope-hint").textContent(), /Portée invalide/);
    await page.fill("#ai-pages", "2-3");
    assert.match(await page.locator("#ai-scope-hint").textContent(), /2 pages choisies/);

    await page.fill("#ai-question", "Quelle est la durée du contrat ?");
    await page.press("#ai-question", "Enter");
    // Plafond : refusé avant tout envoi.
    await page.locator("#toast.error", { hasText: "Plafond mensuel" }).waitFor();
    assert.equal((await app.callsTo("ai_start")).length, 0);
    const [estimate] = await app.callsTo("ai_estimate");
    assert.deepEqual(estimate.scope, { kind: "pages", pages: [1, 2] });
    assert.deepEqual(estimate.task, { kind: "question", question: "Quelle est la durée du contrat ?" });

    await page.click("#ai-send");
    await waitForCalls(app, "ai_start");
    await app.emit("ai-event", { run: 9, kind: "reading", done: 1, total: 4 });
    await page.locator(".ai-status", { hasText: "Lecture de la partie 2 sur 4" }).waitFor();
    await page.locator(".ai-answer.running button", { hasText: "Annuler" }).click();
    const [cancel] = await waitForCalls(app, "ai_cancel");
    assert.deepEqual(cancel, { run: 9 });
    await app.emit("ai-done", { run: 9, Err: { kind: "cancelled", message: "action IA annulée" } });
    await page.locator(".ai-answer.cancelled .ai-status", { hasText: "Action annulée" }).waitFor();
  } finally {
    await app.close();
  }
});

test("Réglages IA : préréglage, clé au trousseau, test, profil", async () => {
  const app = await openDocument({
    ai_status: [{ ok: { configured: false, label: "", host: "", has_key: false, local: false } }],
    ai_usage: [{ ok: { today: { requests: 1, cost: 0.002 }, month: { cost: 0.4 }, total: { requests: 12, cost: 1.5 } } }],
    ai_set_key: [{ ok: null }],
    ai_test: [{ ok: { ok: true, message: "Le modèle répond : « OK »", hints: [], models: ["deepseek-flash", "deepseek-pro"] } }],
    save_settings: [{ ok: DEFAULT_SETTINGS }],
  });
  const { page } = app;
  try {
    await page.click("#menu-button");
    await page.click("#menu-settings");
    await page.click('.nav-item[data-section="ai"]');
    await page.locator("#ai-usage", { hasText: "ce mois-ci 0.4000 USD" }).waitFor();
    await page.selectOption("#ai-preset", "deepseek");
    assert.equal(await page.locator("#set-ai-url").inputValue(), "https://api.deepseek.com");
    assert.equal(await page.locator("#set-ai-model").inputValue(), "deepseek-flash");

    await page.fill("#ai-key", "sk-test-factice");
    await page.click("#ai-key-save");
    const [key] = await waitForCalls(app, "ai_set_key");
    assert.equal(key.key, "sk-test-factice");
    assert.equal(key.provider.base_url, "https://api.deepseek.com");
    assert.equal(await page.locator("#ai-key").inputValue(), "", "la clé n'est pas gardée à l'écran");

    await page.click("#ai-test");
    await page.locator("#ai-test-result.ok", { hasText: "Réglages valides" }).waitFor();
    assert.equal(await page.locator("#ai-models option").count(), 2);
    // Modèles du serveur en boutons : un clic choisit le modèle.
    const models = page.locator("#ai-test-result .ai-model-list button");
    assert.deepEqual(await models.allTextContents(), ["deepseek-flash", "deepseek-pro"]);
    await models.nth(1).click();
    assert.equal(await page.inputValue("#set-ai-model"), "deepseek-pro");
    assert.equal(await models.nth(1).getAttribute("aria-pressed"), "true");
    assert.equal(await models.nth(0).getAttribute("aria-pressed"), "false");

    await page.fill("#ai-profile-name", "DeepSeek");
    await page.click("#ai-profile-save");
    assert.equal(await page.locator("#set-ai-fallback option").count(), 2);
    await page.selectOption("#set-ai-fallback", "DeepSeek");
    await page.click("#settings-save");
    const [saved] = await waitForCalls(app, "save_settings");
    assert.equal(saved.settings.ai.provider.base_url, "https://api.deepseek.com");
    assert.equal(saved.settings.ai.provider.model, "deepseek-pro");
    assert.equal(saved.settings.ai.profiles[0].name, "DeepSeek");
    assert.equal(saved.settings.ai.fallback, "DeepSeek");
    assert.equal(JSON.stringify(saved).includes("sk-test"), false, "la clé n'est jamais dans les réglages");
  } finally {
    await app.close();
  }
});

test("Panneau IA : bas du panneau utilisable, largeur réglable, réponse supprimable", async () => {
  const app = await openDocument({ ai_estimate: [{ ok: { ...ESTIMATE, needs_confirmation: false } }], ai_start: [{ ok: 5 }] });
  const { page } = app;
  try {
    await page.click("#mode-ai");
    // Type de demande, champ et bouton : chacun assez large pour servir.
    const mode = await page.locator(".ai-ask-mode").boundingBox();
    const field = await page.locator("#ai-question").boundingBox();
    const send = await page.locator("#ai-send").boundingBox();
    assert.ok(mode.y + mode.height <= field.y + 1, "le type de demande est au-dessus du champ");
    assert.ok(field.width > 180, `champ trop étroit : ${field.width}`);
    assert.ok(send.x >= field.x + field.width - 1, "le bouton est à côté du champ");

    // Poignée du bord gauche : tirer vers la gauche élargit le panneau.
    const before = (await page.locator("#ai-panel").boundingBox()).width;
    const handle = await page.locator("#ai-resizer").boundingBox();
    await page.mouse.move(handle.x + 2, handle.y + 200);
    await page.mouse.down();
    await page.mouse.move(handle.x - 150, handle.y + 200, { steps: 5 });
    await page.mouse.up();
    const after = (await page.locator("#ai-panel").boundingBox()).width;
    assert.ok(after > before + 100, `${before} → ${after}`);
    // Au clavier aussi.
    await page.focus("#ai-resizer");
    await page.keyboard.press("ArrowRight");
    assert.ok((await page.locator("#ai-panel").boundingBox()).width < after);

    // Une réponse finie se supprime ; pas pendant qu'elle s'écrit.
    await page.click('[data-task="summary"]');
    await waitForCalls(app, "ai_start");
    const answer = page.locator(".ai-answer").first();
    assert.equal(await answer.locator(".ai-remove").isVisible(), false);
    await app.emit("ai-done", { run: 5, Ok: { text: "Résumé.", cost: 0, currency: "USD", model: "m", fallback_used: false } });
    await answer.locator(".ai-remove").click();
    assert.equal(await page.locator(".ai-answer").count(), 0);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
