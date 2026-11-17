// Sauvegarde automatique et récupération après plantage (F-02), réglage
// Fichiers (R-04).

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

const RECOVERY = {
  id: "1700000000000-42-0/doc-1",
  file_name: "contrat.pdf",
  path: "/docs/contrat.pdf",
  saved_at: Math.floor(Date.now() / 1000) - 300,
  elements: 2,
};

const TEXT = {
  kind: "text",
  page: 0,
  top_left: { x: 100, y: 200 },
  width: 200,
  text: "Annexe B",
  style: { font: "sans", size: 11, bold: false, italic: false, color: { r: 0, g: 0, b: 0 }, align: "left", line_height: 1.25 },
};
const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };
const RESTORED = {
  elements: [{ id: 1, element: TEXT, bounds: { x: 100, y: 200, width: 200, height: 14 } }],
  revisions: [1, 0, 0],
  can_undo: true,
  can_redo: false,
  dirty: true,
};

async function startWithRecovery(commands = {}) {
  const app = await launch({
    recoveries: [{ ok: [RECOVERY] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    ...commands,
  });
  await app.page.locator("#recovery-dialog[open]").waitFor();
  return app;
}

test("après un plantage : Récupérer rouvre le fichier et reprend ses ajouts", async () => {
  const app = await startWithRecovery({ restore_recovery: [{ ok: RESTORED }] });
  try {
    const text = await app.page.textContent("#recovery-text");
    assert.match(text, /« contrat\.pdf »/);
    assert.match(text, /2 élément\(s\)/);
    assert.match(text, /Copie de secours : il y a 5 min, 2 élément\(s\) ajouté\(s\)\./);
    await app.page.click("#recovery-restore");
    await app.page.locator(".edit-layer .element").first().waitFor();
    assert.deepEqual(await app.callsTo("open_document"), [{ path: "/docs/contrat.pdf", password: null }]);
    assert.deepEqual(await app.callsTo("restore_recovery"), [{ id: 1, recoveryId: RECOVERY.id }]);
    // Reprise en mode Éditer, document marqué modifié, rappel d'enregistrer.
    assert.equal(await app.page.locator("#mode-edit").getAttribute("aria-current"), "page");
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), true);
    assert.match(await app.page.textContent("#save-state"), /copie de secours à \d\d:\d\d/);
    assert.match(await app.page.textContent("#toast"), /reprises\. Pensez à enregistrer/);
    // La reprise passe avant le fichier passé en argument ou le dernier document.
    assert.deepEqual(await app.callsTo("initial_files"), []);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Ignorer efface la copie de secours, rien n'est ouvert", async () => {
  const app = await startWithRecovery();
  try {
    await app.page.click("#recovery-discard");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "discard_recovery"));
    assert.deepEqual(await app.callsTo("discard_recovery"), [{ recoveryId: RECOVERY.id }]);
    assert.deepEqual(await app.callsTo("open_document"), []);
    await app.page.locator("#home:visible").waitFor();
  } finally {
    await app.close();
  }
});

test("Plus tard ou Échap : la copie est gardée, le démarrage continue", async () => {
  const app = await startWithRecovery({ initial_files: [{ ok: ["/docs/autre.pdf"] }] });
  try {
    await app.page.keyboard.press("Escape");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "open_document"));
    assert.deepEqual(await app.callsTo("discard_recovery"), []);
    assert.deepEqual(await app.callsTo("restore_recovery"), []);
    assert.deepEqual(await app.callsTo("open_document"), [{ path: "/docs/autre.pdf", password: null }]);
  } finally {
    await app.close();
  }
});

test("fichier changé depuis : la reprise est refusée avec son explication", async () => {
  const message = "« contrat.pdf » a changé depuis la sauvegarde automatique : ses modifications ne peuvent pas y être reprises";
  const app = await startWithRecovery({ restore_recovery: [{ error: { kind: "invalid_request", message } }] });
  try {
    await app.page.click("#recovery-restore");
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /Reprise impossible : « contrat\.pdf » a changé/);
  } finally {
    await app.close();
  }
});

test("copie de secours écrite : son heure s'affiche dans la barre d'état", async () => {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: RESTORED }],
  });
  try {
    await app.page.locator("#doc-dirty:visible").waitFor();
    assert.equal(await app.page.textContent("#save-state"), "Modifications non enregistrées");
    // Un autre document : rien ne change.
    await app.emit("autosaved", { documents: [9] });
    assert.equal(await app.page.textContent("#save-state"), "Modifications non enregistrées");
    await app.emit("autosaved", { documents: [1] });
    assert.match(await app.page.textContent("#save-state"), /^Modifications non enregistrées · copie de secours à \d\d:\d\d$/);
  } finally {
    await app.close();
  }
});

test("Réglages, Fichiers : intervalle de la sauvegarde automatique", async () => {
  const custom = structuredClone(DEFAULT_SETTINGS);
  custom.files.autosave_seconds = 45;
  const saved = structuredClone(DEFAULT_SETTINGS);
  saved.files.autosave_seconds = 300;
  const app = await launch({
    get_settings: [{ ok: { settings: custom, warnings: [] } }],
    save_settings: [{ ok: saved }],
  });
  try {
    await app.page.click("#menu-button");
    await app.page.click("#menu-settings");
    await app.page.click(".settings-nav [data-section=files]");
    // Valeur écrite à la main dans config.toml : affichée telle quelle.
    assert.equal(await app.page.inputValue("#set-autosave"), "45");
    assert.match(await app.page.locator("#set-autosave option:checked").textContent(), /45 secondes/);
    await app.page.selectOption("#set-autosave", "300");
    await app.page.click("#settings-save");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "save_settings"));
    const [save] = await app.callsTo("save_settings");
    assert.equal(save.settings.files.autosave_seconds, 300);
  } finally {
    await app.close();
  }
});

test("liste des récupérations absente : le démarrage continue", async () => {
  const app = await launch({
    recoveries: [{ ok: null }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
