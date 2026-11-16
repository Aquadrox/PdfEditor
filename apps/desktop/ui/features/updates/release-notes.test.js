import assert from "node:assert/strict";
import { test } from "node:test";

import { releaseTitle } from "./release-notes.js";

test("titre d'une version, avec ou sans date", () => {
  assert.equal(releaseTitle({ version: "0.0.5", date: "16 novembre 2026" }), "Version 0.0.5 du 16 novembre 2026");
  assert.equal(releaseTitle({ version: "0.0.5" }), "Version 0.0.5");
});
