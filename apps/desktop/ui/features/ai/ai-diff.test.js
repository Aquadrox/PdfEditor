import assert from "node:assert/strict";
import { test } from "node:test";

import { wordDiff } from "./ai-diff.js";

const kinds = (parts) => parts.map((p) => `${p.kind}:${p.text}`);

test("un mot remplacé", () => {
  assert.deepEqual(kinds(wordDiff("Salut à vous", "Bonjour à vous")), ["del:Salut", "ins:Bonjour", "same: à vous"]);
});

test("ajout et suppression en fin de ligne", () => {
  assert.deepEqual(kinds(wordDiff("Le contrat", "Le contrat signé")), ["same:Le contrat", "ins: signé"]);
  assert.deepEqual(kinds(wordDiff("Le contrat signé", "Le contrat")), ["same:Le contrat", "del: signé"]);
});

test("identique ou vide", () => {
  assert.deepEqual(kinds(wordDiff("abc", "abc")), ["same:abc"]);
  assert.deepEqual(kinds(wordDiff("", "Nouveau")), ["ins:Nouveau"]);
  assert.deepEqual(wordDiff("", ""), []);
});

test("les deux textes se reconstituent", () => {
  const before = "Les parties conviennent de ce qui suit, sans réserve.";
  const after = "Les deux parties conviennent expressément de ce qui suit.";
  const parts = wordDiff(before, after);
  assert.equal(parts.filter((p) => p.kind !== "ins").map((p) => p.text).join(""), before);
  assert.equal(parts.filter((p) => p.kind !== "del").map((p) => p.text).join(""), after);
});
