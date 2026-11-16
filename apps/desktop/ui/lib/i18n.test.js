import assert from "node:assert/strict";
import { test } from "node:test";

import { t } from "./i18n.js";
import fr from "./messages-fr.js";

test("paramètres remplacés", () => {
  assert.equal(t("thumb.label", { n: 3 }), "Page 3");
  assert.equal(t("status.of", { count: 12 }), "sur 12");
});

test("paramètre absent laissé visible, clé inconnue renvoyée telle quelle", () => {
  assert.equal(t("thumb.label"), "Page {n}");
  assert.equal(t("cle.inconnue"), "cle.inconnue");
});

test("aucun texte vide dans le catalogue", () => {
  for (const [key, value] of Object.entries(fr)) {
    assert.ok(value.trim().length > 0, key);
  }
});
