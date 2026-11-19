import assert from "node:assert/strict";
import { test } from "node:test";

import { readRequest } from "./new-document.js";

test("demande de nouveau document bornée", () => {
  assert.deepEqual(readRequest("a4", "portrait", "3"), { paper: "a4", landscape: false, pages: 3 });
  assert.deepEqual(readRequest("letter", "landscape", "0"), { paper: "letter", landscape: true, pages: 1 });
  assert.deepEqual(readRequest("a5", "portrait", "9999"), { paper: "a5", landscape: false, pages: 200 });
  assert.deepEqual(readRequest("a4", "portrait", "abc"), { paper: "a4", landscape: false, pages: 1 });
});
