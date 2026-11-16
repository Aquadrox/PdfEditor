// Ouverture d'un document protégé par mot de passe (V-02).

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const LOCKED = { error: { kind: "password_required", message: "protégé" } };
const WRONG = { error: { kind: "wrong_password", message: "mot de passe incorrect" } };

async function openLocked(answers) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/secret.pdf"] }],
    open_document: answers,
  });
  await app.page.locator("#password-dialog[open]").waitFor();
  return app;
}

// Non-régression : Entrée déclenchait « Annuler », premier bouton du formulaire.
test("Entrée valide le mot de passe saisi", async () => {
  const app = await openLocked([LOCKED, { ok: documentView() }]);
  try {
    await app.page.fill("#password-input", "secret");
    await app.page.keyboard.press("Enter");
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    const opens = (await app.calls()).filter((c) => c.command === "open_document");
    assert.deepEqual(opens.at(-1).args, { path: "/docs/secret.pdf", password: "secret" });
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("mauvais mot de passe : message puis nouvel essai", async () => {
  const app = await openLocked([LOCKED, WRONG, { ok: documentView() }]);
  try {
    await app.page.fill("#password-input", "faux");
    await app.page.keyboard.press("Enter");
    await app.page.locator("#password-error:visible").waitFor();
    assert.equal(await app.page.inputValue("#password-input"), "");
    await app.page.fill("#password-input", "bon");
    await app.page.click("#password-dialog button.primary");
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  } finally {
    await app.close();
  }
});

test("Annuler ferme sans ouvrir le document", async () => {
  const app = await openLocked([LOCKED]);
  try {
    await app.page.fill("#password-input", "abandon");
    await app.page.click("#password-cancel");
    await app.page.locator("#password-dialog:not([open])").waitFor({ state: "attached" });
    assert.equal(await app.page.locator("#home").isVisible(), true);
    const opens = (await app.calls()).filter((c) => c.command === "open_document");
    assert.equal(opens.length, 1);
  } finally {
    await app.close();
  }
});
