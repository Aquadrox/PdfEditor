// Mises à jour : annonce, nouveautés, installation sur clic seulement.

import assert from "node:assert/strict";
import { test } from "node:test";

import { launch } from "./harness.js";

const NOTES = [{ version: "0.0.6", date: "20 novembre 2026", items: ["Signature des documents."] }];
const AVAILABLE = { current: "0.0.5", available: "0.0.6", notes: NOTES, configured: true };

test("version disponible : bandeau, nouveautés, installation sur clic", async () => {
  const app = await launch({ update_status: [{ ok: AVAILABLE }] });
  try {
    await app.page.locator("#update-banner:visible").waitFor();
    assert.equal(await app.page.textContent("#update-text"), "La version 0.0.6 est disponible.");
    assert.equal((await app.calls()).some((c) => c.command === "install_update"), false);

    await app.page.click("#update-notes-toggle");
    assert.equal(await app.page.textContent("#update-notes .release-title"), "Version 0.0.6 du 20 novembre 2026");
    assert.equal(await app.page.textContent("#update-notes li"), "Signature des documents.");

    await app.page.click("#update-install");
    assert.ok((await app.calls()).some((c) => c.command === "install_update"));
  } finally {
    await app.close();
  }
});

test("échec de l'installation expliqué, bouton réutilisable", async () => {
  const app = await launch({
    update_status: [{ ok: AVAILABLE }],
    install_update: [{ error: "signature invalide" }],
  });
  try {
    await app.page.click("#update-install");
    await app.page.locator("#update-text", { hasText: "Mise à jour impossible : signature invalide" }).waitFor();
    assert.equal(await app.page.isDisabled("#update-install"), false);
  } finally {
    await app.close();
  }
});

test("après une mise à jour : nouveautés jusqu'à fermeture du bandeau", async () => {
  const app = await launch({
    changelog_view: [{ ok: { current: "0.0.6", releases: NOTES, updated_from: "0.0.5", news: NOTES } }],
  });
  try {
    await app.page.locator("#updated-banner:visible").waitFor();
    assert.equal(
      await app.page.textContent("#updated-text"),
      "PdfEditor a été mis à jour en version 0.0.6 (depuis la 0.0.5).",
    );
    await app.page.click("#updated-close");
    assert.equal(await app.page.locator("#updated-banner").isVisible(), false);
    assert.ok((await app.calls()).some((c) => c.command === "dismiss_update_news"));
  } finally {
    await app.close();
  }
});

test("à jour : aucun bandeau ; recherche depuis les réglages", async () => {
  const app = await launch({
    check_update: [{ ok: { current: "0.0.5", available: null, notes: [], configured: true } }],
  });
  try {
    await app.page.locator("#home-open").waitFor();
    assert.equal(await app.page.locator("#update-banner").isVisible(), false);
    await app.page.click("#menu-button");
    await app.page.click("#menu-settings");
    await app.page.click(".settings-nav [data-section=diagnostic]");
    await app.page.click("#update-check");
    await app.page.locator("#update-check-status", { hasText: "PdfEditor est à jour (version 0.0.5)." }).waitFor();
  } finally {
    await app.close();
  }
});
