import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { test } from "node:test";

import { normalize, queryWords, searchActions } from "./palette-search.js";

// Le vrai catalogue, tel que l'envoie l'application (vérifié par pde-app).
const CATALOG = JSON.parse(
  readFileSync(new URL("../../../ui-tests/fixtures/action-catalog.json", import.meta.url), "utf8"),
);
const first = (query) => searchActions(CATALOG, query)[0]?.id;

test("normalisation sans accents ni ponctuation", () => {
  assert.equal(normalize("Éditer : « Pivoter » !"), "editer pivoter");
  assert.deepEqual(queryWords("Comment je fais pour tourner les pages ?"), ["tourner", "page"]);
});

test("recherche vide : tout le catalogue dans l'ordre", () => {
  assert.deepEqual(
    searchActions(CATALOG, "  ").map((a) => a.id),
    CATALOG.map((a) => a.id),
  );
});

test("le nom, un début de mot ou un mot-clé trouvent l'action", () => {
  assert.equal(first("enregistrer"), "save");
  assert.equal(first("pivot"), "rotate_pages");
  assert.equal(first("zoom avant"), "zoom_in");
  assert.equal(first("caviarder"), "redact");
});

test("« Comment faire ? » : une question en mots courants", () => {
  assert.equal(first("comment tourner une page ?"), "rotate_pages");
  assert.equal(first("fusionner deux PDF"), "insert_pdf");
  // L'anonymisation assistée d'abord, le caviardage à la main juste après.
  assert.deepEqual(
    searchActions(CATALOG, "anonymiser un nom").slice(0, 2).map((a) => a.id),
    ["ai_anonymize", "redact"],
  );
  assert.equal(first("mettre un commentaire"), "add_note");
  assert.equal(first("corriger une faute"), "edit_text");
  assert.equal(first("séparer un PDF en plusieurs"), "extract_pages");
  assert.equal(first("changer les raccourcis"), "settings");
});

test("« masquer » propose le caviardage et le masque", () => {
  const ids = searchActions(CATALOG, "masquer").map((a) => a.id);
  assert.ok(ids.includes("redact") && ids.includes("add_shape"), ids.join(", "));
});

test("une recherche sans rapport ne trouve rien", () => {
  assert.deepEqual(searchActions(CATALOG, "xylophone"), []);
});
