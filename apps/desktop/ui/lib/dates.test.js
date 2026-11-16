import assert from "node:assert/strict";
import { test } from "node:test";

import { relativeDate } from "./dates.js";

// Dates locales : le test ne dépend pas du fuseau de la machine.
const at = (y, m, d, h = 12, min = 0) => new Date(y, m - 1, d, h, min).getTime() / 1000;
const now = at(2026, 11, 16, 14, 30);

test("quelques minutes", () => {
  assert.equal(relativeDate(now - 20, now), "à l'instant");
  assert.equal(relativeDate(now - 10 * 60, now), "il y a 10 min");
});

test("aujourd'hui et hier, avec l'heure", () => {
  assert.equal(relativeDate(at(2026, 11, 16, 9, 5), now), "aujourd'hui à 09:05");
  assert.equal(relativeDate(at(2026, 11, 15, 18, 0), now), "hier à 18:00");
});

test("plus ancien : la date, et l'année si elle diffère", () => {
  assert.equal(relativeDate(at(2026, 11, 12), now), "12 novembre");
  assert.equal(relativeDate(at(2025, 3, 2), now), "2 mars 2025");
});
