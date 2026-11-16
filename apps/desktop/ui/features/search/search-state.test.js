import assert from "node:assert/strict";
import { test } from "node:test";

import { countLabel, firstFrom, step } from "./search-state.js";

test("suivante et précédente bouclent", () => {
  assert.equal(step(0, 3, 1), 1);
  assert.equal(step(2, 3, 1), 0);
  assert.equal(step(0, 3, -1), 2);
  assert.equal(step(0, 0, 1), -1);
});

test("la recherche part de la page affichée", () => {
  const hits = [{ page: 0 }, { page: 2 }, { page: 5 }];
  assert.equal(firstFrom(hits, 0), 0);
  assert.equal(firstFrom(hits, 3), 2);
  // Après la dernière occurrence : retour au début.
  assert.equal(firstFrom(hits, 9), 0);
  assert.equal(firstFrom([], 0), -1);
});

test("libellé du nombre d'occurrences", () => {
  assert.equal(countLabel(2, 12, false), "3 sur 12");
  assert.equal(countLabel(0, 1000, true), "1 sur 1000+");
  assert.equal(countLabel(-1, 0, false), "Aucun résultat");
});
