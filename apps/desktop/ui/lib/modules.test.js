// Chaque module de l'interface se lit sans erreur de syntaxe : une seule
// faute empêche toute l'application de démarrer, et les tests purs ne
// chargent pas tous les modules (ceux qui touchent au DOM).

import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";
import { test } from "node:test";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("..", import.meta.url));

function modules(dir) {
  return readdirSync(dir).flatMap((name) => {
    const path = join(dir, name);
    if (statSync(path).isDirectory()) return name === "node_modules" || name === "fonts" ? [] : modules(path);
    return name.endsWith(".js") && !name.endsWith(".test.js") ? [path] : [];
  });
}

test("tous les modules de l'interface ont une syntaxe valide", () => {
  const files = modules(root);
  assert.ok(files.length > 30, `${files.length} modules trouvés`);
  for (const file of files) {
    assert.doesNotThrow(() => execFileSync(process.execPath, ["--check", file], { stdio: "pipe" }), file);
  }
});
