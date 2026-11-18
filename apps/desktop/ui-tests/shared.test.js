// Android (A-02) : document reçu d'une autre application (« Ouvrir avec »,
// « Partager ») : ouvert au démarrage, ou au retour au premier plan si
// l'application tournait déjà. Sur PC, la commande ne rend rien.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const COPY = "/data/user/0/fr.aquadrox.pdfeditor/documents/bail.pdf";

function commands(extra = {}) {
  return {
    open_document: [{ ok: { ...documentView(1), file_name: "bail.pdf" } }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ...extra,
  };
}

test("document reçu au démarrage : ouvert", async () => {
  const app = await launch(commands({ shared_files: [{ ok: [COPY] }] }));
  try {
    await app.page.locator("#doc-name", { hasText: "bail.pdf" }).waitFor();
    const [opened] = await app.callsTo("open_document");
    assert.equal(opened.path, COPY);
  } finally {
    await app.close();
  }
});

test("document reçu pendant que l'application tourne : ouvert au retour", async () => {
  const app = await launch(commands({ shared_files: [{ ok: [] }, { ok: [COPY] }, { ok: [] }] }));
  const { page } = app;
  try {
    await page.locator("#home:not([hidden]), .home:not([hidden])").first().waitFor();
    assert.equal((await app.callsTo("open_document")).length, 0);
    await page.evaluate(() => document.dispatchEvent(new Event("visibilitychange")));
    await page.locator("#doc-name", { hasText: "bail.pdf" }).waitFor();
    assert.equal((await app.callsTo("open_document"))[0].path, COPY);
  } finally {
    await app.close();
  }
});
