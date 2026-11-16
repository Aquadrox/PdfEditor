import assert from "node:assert/strict";
import { test } from "node:test";

import { countChanges, getPath, setPath } from "./settings-state.js";

const settings = { general: { theme: "system", recent_files: 10 }, shortcuts: { open: "Ctrl+O" } };

test("lecture et écriture par chemin, sans modifier l'original", () => {
  assert.equal(getPath(settings, "general.theme"), "system");
  const changed = setPath(settings, "general.theme", "dark");
  assert.equal(changed.general.theme, "dark");
  assert.equal(settings.general.theme, "system");
});

test("nombre de réglages modifiés", () => {
  assert.equal(countChanges(settings, structuredClone(settings)), 0);
  let draft = setPath(settings, "general.theme", "dark");
  draft = setPath(draft, "shortcuts.open", "Ctrl+Shift+O");
  assert.equal(countChanges(settings, draft), 2);
});
