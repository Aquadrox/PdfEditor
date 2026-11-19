// Page Réglages (R-01, R-02, R-08, R-09) et application des réglages.

import assert from "node:assert/strict";
import { test } from "node:test";

import { DEFAULT_SETTINGS, documentView, launch } from "./harness.js";

function withSettings(change) {
  const settings = structuredClone(DEFAULT_SETTINGS);
  change(settings);
  return settings;
}

async function openSettings(app) {
  await app.page.click("#menu-button");
  await app.page.click("#menu-settings");
  await app.page.locator("#settings-view").waitFor();
}

test("changer le thème : brouillon compté, enregistré, appliqué", async () => {
  const dark = withSettings((s) => (s.general.theme = "dark"));
  const app = await launch({ save_settings: [{ ok: dark }] });
  try {
    await openSettings(app);
    assert.equal(await app.page.isDisabled("#settings-save"), true);
    await app.page.selectOption("#set-theme", "dark");
    assert.equal(await app.page.textContent("#settings-dirty"), "1 réglage(s) modifié(s)");
    await app.page.click("#settings-save");
    await app.page.waitForFunction(() => document.documentElement.dataset.theme === "dark");
    const save = (await app.calls()).find((c) => c.command === "save_settings");
    assert.equal(save.args.settings.general.theme, "dark");
    assert.equal(await app.page.textContent("#settings-dirty"), "");
  } finally {
    await app.close();
  }
});

test("Annuler remet les valeurs enregistrées", async () => {
  const app = await launch();
  try {
    await openSettings(app);
    await app.page.selectOption("#set-theme", "light");
    await app.page.click("#settings-cancel");
    assert.equal(await app.page.inputValue("#set-theme"), "system");
    assert.equal(await app.page.isDisabled("#settings-save"), true);
  } finally {
    await app.close();
  }
});

test("saisir un raccourci, refuser un doublon", async () => {
  const app = await launch();
  try {
    await openSettings(app);
    await app.page.click(".settings-nav [data-section=shortcuts]");
    const search = app.page.locator("#shortcuts-body tr", { hasText: "Rechercher" }).locator("button");
    await search.click();
    await app.page.keyboard.press("Control+Shift+K");
    assert.equal(await search.textContent(), "Ctrl+Maj+K");

    await search.click();
    await app.page.keyboard.press("Control+O");
    assert.match(await app.page.textContent("#shortcuts-body"), /Déjà utilisé par « Ouvrir un document »/);
    assert.equal(await search.textContent(), "Ctrl+Maj+K");
    assert.equal(await app.page.textContent("#settings-dirty"), "1 réglage(s) modifié(s)");
  } finally {
    await app.close();
  }
});

test("un raccourci modifié remplace l'ancien", async () => {
  const settings = withSettings((s) => (s.shortcuts.search = "F3"));
  const app = await launch({
    get_settings: [{ ok: { settings, warnings: [] } }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(2) }],
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    await app.page.keyboard.press("Control+f");
    assert.equal(await app.page.locator("#search-bar").isVisible(), false);
    await app.page.keyboard.press("F3");
    await app.page.locator("#search-bar").waitFor();
    assert.match(await app.page.getAttribute("#search-button", "title"), /\(F3\)$/);
  } finally {
    await app.close();
  }
});

test("zoom d'ouverture réglé sur la taille réelle", async () => {
  const settings = withSettings((s) => (s.display.default_zoom = "actual"));
  const app = await launch({
    get_settings: [{ ok: { settings, warnings: [] } }],
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(1) }],
  });
  try {
    await app.page.locator("#zoom-value", { hasText: "100" }).waitFor();
  } finally {
    await app.close();
  }
});

test("avertissements de lecture de config.toml affichés", async () => {
  const app = await launch({
    get_settings: [{ ok: { settings: DEFAULT_SETTINGS, warnings: ["config.toml illisible"] } }],
  });
  try {
    await app.page.locator("#toast:visible", { hasText: "config.toml illisible" }).waitFor();
    await openSettings(app);
    assert.equal(await app.page.textContent("#settings-warnings"), "config.toml illisible");
  } finally {
    await app.close();
  }
});

test("Fichiers : dossier par défaut choisi dans le dialogue, suffixe des copies", async () => {
  const saved = withSettings((s) => {
    s.files.default_dir = "/home/moi/Documents";
    s.files.copy_suffix = " - relu";
  });
  const app = await launch({ pick_folder: [{ ok: "/home/moi/Documents" }], save_settings: [{ ok: saved }] });
  try {
    await openSettings(app);
    await app.page.click(".settings-nav [data-section=files]");
    assert.equal(await app.page.inputValue("#set-copy-suffix"), " (modifié)");
    assert.equal(await app.page.getAttribute("#set-default-dir", "placeholder"), "Celui du système");
    await app.page.click("#pick-default-dir");
    await app.page.waitForFunction(() => document.getElementById("set-default-dir").value === "/home/moi/Documents");
    await app.page.fill("#set-copy-suffix", " - relu");
    await app.page.press("#set-copy-suffix", "Tab");
    assert.equal(await app.page.textContent("#settings-dirty"), "2 réglage(s) modifié(s)");
    await app.page.click("#settings-save");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "save_settings"));
    const save = (await app.calls()).find((c) => c.command === "save_settings");
    assert.equal(save.args.settings.files.default_dir, "/home/moi/Documents");
    assert.equal(save.args.settings.files.copy_suffix, " - relu");
  } finally {
    await app.close();
  }
});

test("Fichiers : dialogue du dossier annulé, rien ne change", async () => {
  const app = await launch({ pick_folder: [{ ok: null }] });
  try {
    await openSettings(app);
    await app.page.click(".settings-nav [data-section=files]");
    await app.page.click("#pick-default-dir");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "pick_folder"));
    assert.equal(await app.page.inputValue("#set-default-dir"), "");
    assert.equal(await app.page.isDisabled("#settings-save"), true);
  } finally {
    await app.close();
  }
});

test("Lecteur PDF par défaut : bouton sous Windows seulement, ouvre les Paramètres de Windows", async () => {
  const windows = await launch({ app_info: [{ ok: { version: "0.0.0-test", os: "windows" } }], open_default_apps: [{ ok: null }] });
  try {
    await openSettings(windows);
    await windows.page.locator("#default-app").waitFor({ state: "visible" });
    await windows.page.click("#default-app");
    await windows.page.waitForFunction(() => window.__calls.some((c) => c.command === "open_default_apps"));
    assert.deepEqual(windows.errors, []);
  } finally {
    await windows.close();
  }
  const linux = await launch({ app_info: [{ ok: { version: "0.0.0-test", os: "linux" } }] });
  try {
    await openSettings(linux);
    assert.equal(await linux.page.isHidden("#default-app-field"), true);
  } finally {
    await linux.close();
  }
});
