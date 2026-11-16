// Visionneuse : document affiché, navigation, zoom, fichiers refusés.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

async function openDocument(pageCount) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(pageCount) }],
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

test("le document remplace l'accueil, une page par feuille", async () => {
  const app = await openDocument(3);
  try {
    assert.equal(await app.page.locator("#home").isVisible(), false);
    assert.equal(await app.page.locator(".page").count(), 3);
    assert.equal(await app.page.locator(".thumb").count(), 3);
    assert.equal(await app.page.textContent("#page-count"), "sur 3");
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("aller à une page par son numéro", async () => {
  const app = await openDocument(5);
  try {
    await app.page.fill("#page-input", "4");
    await app.page.press("#page-input", "Enter");
    await app.page.waitForFunction(() => document.querySelectorAll(".thumb")[3].getAttribute("aria-current") === "page");
  } finally {
    await app.close();
  }
});

test("zoom avant, arrière et 100 %", async () => {
  const app = await openDocument(2);
  try {
    await app.page.keyboard.press("Control+0");
    assert.equal(await app.page.textContent("#zoom-value"), "100 %");
    await app.page.click("#zoom-in");
    assert.equal(await app.page.textContent("#zoom-value"), "110 %");
    await app.page.click("#zoom-out");
    await app.page.click("#zoom-out");
    assert.equal(await app.page.textContent("#zoom-value"), "90 %");
  } finally {
    await app.close();
  }
});

test("un fichier qui n'est pas un PDF est refusé avec un message", async () => {
  const app = await launch({ initial_files: [{ ok: ["/docs/photo.jpg"] }] });
  try {
    await app.page.locator("#toast:visible").waitFor();
    assert.match(await app.page.textContent("#toast"), /PDF/);
    const opens = (await app.calls()).filter((c) => c.command === "open_document");
    assert.equal(opens.length, 0);
  } finally {
    await app.close();
  }
});

test("erreur d'ouverture expliquée, l'accueil reste affiché", async () => {
  const app = await launch({
    initial_files: [{ ok: ["/docs/casse.pdf"] }],
    open_document: [{ error: { kind: "invalid_file", message: "ce fichier n'est pas un PDF lisible" } }],
  });
  try {
    await app.page.locator("#toast:visible").waitFor();
    assert.match(await app.page.textContent("#toast"), /casse\.pdf.*pas un PDF lisible/);
    assert.equal(await app.page.locator("#home").isVisible(), true);
  } finally {
    await app.close();
  }
});
