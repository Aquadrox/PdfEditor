// Documents récents sur l'accueil.

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

const now = () => Math.floor(Date.now() / 1000);
const RECENT = [
  { path: "/docs/contrat.pdf", name: "contrat.pdf", folder: "/docs", opened_at: now() - 600, exists: true },
  { path: "/vieux/parti.pdf", name: "parti.pdf", folder: "/vieux", opened_at: now() - 86400 * 40, exists: false },
];

test("liste affichée, document introuvable désactivé, clic pour ouvrir", async () => {
  const app = await launch({ recent_files: [{ ok: RECENT }], open_document: [{ ok: documentView(1) }] });
  try {
    await app.page.locator("#recent:visible").waitFor();
    assert.deepEqual(await app.page.locator(".recent-name").allTextContents(), ["contrat.pdf", "parti.pdf"]);
    assert.equal(await app.page.locator(".recent-date").first().textContent(), "il y a 10 min");
    assert.equal(await app.page.locator(".recent").nth(1).isDisabled(), true);
    await app.page.locator(".recent").first().click();
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    const open = (await app.calls()).find((c) => c.command === "open_document");
    assert.equal(open.args.path, "/docs/contrat.pdf");
  } finally {
    await app.close();
  }
});

test("pas de liste quand il n'y a aucun document récent", async () => {
  const app = await launch();
  try {
    await app.page.locator("#home-open").waitFor();
    assert.equal(await app.page.locator("#recent").isVisible(), false);
  } finally {
    await app.close();
  }
});

test("rouvrir le dernier document au démarrage", async () => {
  const settings = structuredClone(DEFAULT_SETTINGS);
  settings.general.reopen_last = true;
  const app = await launch({
    get_settings: [{ ok: { settings, warnings: [] } }],
    recent_files: [{ ok: RECENT }],
    open_document: [{ ok: documentView(1) }],
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  } finally {
    await app.close();
  }
});
