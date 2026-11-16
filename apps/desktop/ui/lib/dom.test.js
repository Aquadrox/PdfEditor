import assert from "node:assert/strict";
import { test } from "node:test";

import { fileName } from "./dom.js";

test("nom de fichier sous Windows et Linux", () => {
  assert.equal(fileName("C:\\Users\\moi\\Documents\\contrat.pdf"), "contrat.pdf");
  assert.equal(fileName("/home/moi/devis.pdf"), "devis.pdf");
  assert.equal(fileName("seul.pdf"), "seul.pdf");
});
