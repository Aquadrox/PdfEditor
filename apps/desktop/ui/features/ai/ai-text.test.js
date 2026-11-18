import assert from "node:assert/strict";
import { test } from "node:test";

import { applyPreset, presetFor } from "./ai-presets.js";
import { blocks, inline, parsePages } from "./ai-text.js";

test("Pages choisies : intervalles, doublons, bornes", () => {
  assert.deepEqual(parsePages("2-4, 3; 7", 10), [1, 2, 3, 6]);
  assert.deepEqual(parsePages(" 1 ", 1), [0]);
  assert.equal(parsePages("0-2", 10), null);
  assert.equal(parsePages("5-3", 10), null);
  assert.equal(parsePages("11", 10), null);
  assert.equal(parsePages("deux", 10), null);
  assert.equal(parsePages("", 10), null);
});

test("Citations vérifiées contre le nombre de pages", () => {
  const parts = inline("Durée de deux ans [p. 3] et prix [p. 4-5, 12].", 10);
  assert.deepEqual(parts.map((p) => p.kind), ["text", "cite", "text", "cite", "text"]);
  assert.deepEqual(parts[1], { kind: "cite", text: "[p. 3]", pages: [3], valid: true });
  assert.deepEqual(parts[3].pages, [4, 5, 12]);
  assert.equal(parts[3].valid, false, "page 12 inexistante");
  assert.equal(inline("[pages 2 et 3]", 3)[0].valid, true);
});

test("Markdown réduit : titres, listes, gras", () => {
  const out = blocks("## Résumé\nUn **contrat** [p. 1].\n\n- Durée : 2 ans\n1. Prix", 3);
  assert.deepEqual(out.map((b) => b.kind), ["heading", "paragraph", "item", "item"]);
  assert.deepEqual(out[1].parts.map((p) => p.kind), ["text", "bold", "text", "cite", "text"]);
  // Le HTML du modèle reste du texte.
  assert.deepEqual(blocks("<img src=x onerror=alert(1)>", 1)[0].parts, [{ kind: "text", text: "<img src=x onerror=alert(1)>" }]);
});

test("Préréglages : reconnus par l'adresse, la devise est gardée", () => {
  assert.equal(presetFor("https://api.deepseek.com/").id, "deepseek");
  assert.equal(presetFor("https://exemple.org/v1").id, "custom");
  const provider = applyPreset({ base_url: "", model: "", timeout_seconds: 60, pricing: { currency: "EUR" } }, presetFor("http://localhost:11434/v1"));
  assert.equal(provider.base_url, "http://localhost:11434/v1");
  assert.equal(provider.timeout_seconds, 60);
  assert.equal(provider.pricing.currency, "EUR");
});
