// Texte sélectionnable et recherche dans le document (V-05).

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const TEXT = {
  runs: [
    { text: "Bonjour monde", x: 72, y: 58, width: 160, height: 24 },
    { text: "suite", x: 240, y: 60, width: 50, height: 24 },
    { text: "Deuxième ligne", x: 72, y: 90, width: 150, height: 24 },
  ],
};
const box = (y) => ({ x: 130, y, width: 60, height: 24 });
const TWO_HITS = {
  hits: [
    { page: 0, rects: [box(58)] },
    { page: 1, rects: [box(300)] },
  ],
  truncated: false,
};

async function openWith(searchAnswer) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(2) }],
    page_text: [{ ok: TEXT }],
    search_document: [{ ok: searchAnswer }],
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

test("le texte de la page est posé, sélectionnable", async () => {
  const app = await openWith(TWO_HITS);
  try {
    const layer = app.page.locator(".page").first().locator(".text-layer");
    await layer.waitFor();
    assert.deepEqual(await layer.locator("span").allTextContents(), ["Bonjour monde", "suite", "Deuxième ligne"]);
    assert.equal(await layer.evaluate((l) => getComputedStyle(l).userSelect), "text");
    // Saut de ligne seulement entre deux lignes (y 60 → 90), pas sur la même.
    assert.equal(await layer.locator("br").count(), 1);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Ctrl+F, résultats, suivant, précédent, Échap", async () => {
  const app = await openWith(TWO_HITS);
  const count = () => app.page.textContent("#search-count");
  try {
    await app.page.keyboard.press("Control+f");
    await app.page.locator("#search-input:focus").waitFor();
    await app.page.keyboard.type("monde");
    await app.page.locator("#search-count", { hasText: "1 sur 2" }).waitFor();
    assert.equal(await app.page.locator(".highlight.current").count(), 1);
    const call = (await app.calls()).find((c) => c.command === "search_document");
    assert.deepEqual(call.args, { id: 1, query: "monde" });

    await app.page.keyboard.press("Enter");
    assert.equal(await count(), "2 sur 2");
    await app.page.keyboard.press("Shift+Enter");
    assert.equal(await count(), "1 sur 2");

    await app.page.keyboard.press("Escape");
    await app.page.locator("#search-bar").waitFor({ state: "hidden" });
    assert.equal(await app.page.locator(".highlight").count(), 0);
  } finally {
    await app.close();
  }
});

test("aucun résultat", async () => {
  const app = await openWith({ hits: [], truncated: false });
  try {
    await app.page.click("#search-button");
    await app.page.fill("#search-input", "introuvable");
    await app.page.locator("#search-count", { hasText: "Aucun résultat" }).waitFor();
  } finally {
    await app.close();
  }
});

test("recherche indisponible sans document", async () => {
  const app = await launch();
  try {
    assert.equal(await app.page.isDisabled("#search-button"), true);
    await app.page.keyboard.press("Control+f");
    assert.equal(await app.page.locator("#search-bar").isVisible(), false);
  } finally {
    await app.close();
  }
});
