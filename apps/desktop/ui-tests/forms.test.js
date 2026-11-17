// Formulaire (E-08) en mode Lire, aplatissement, et propriétés du
// document (E-11).

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const at = (x, y, width = 200, height = 20) => ({ page: 0, option: null, bounds: { x, y, width, height } });
const FIELDS = [
  { name: "nom", kind: { kind: "text", multiline: false, max_len: 30 }, read_only: false, value: { kind: "text", value: "Ancien" }, widgets: [at(120, 120)] },
  { name: "accord", kind: { kind: "checkbox" }, read_only: false, value: { kind: "checked", value: false }, widgets: [at(120, 177, 15, 15)] },
  {
    name: "civilite",
    kind: { kind: "radio", options: ["M", "Mme"] },
    read_only: false,
    value: null,
    widgets: [{ ...at(120, 227, 15, 15), option: 0 }, { ...at(160, 227, 15, 15), option: 1 }],
  },
  { name: "ville", kind: { kind: "choice", options: ["Paris", "Lyon"], editable: false }, read_only: false, value: { kind: "choice", value: "Paris" }, widgets: [at(120, 274, 140, 18)] },
  { name: "ref", kind: { kind: "text", multiline: false, max_len: null }, read_only: true, value: null, widgets: [at(300, 120)] },
];

function editView(fields = {}, extra = {}) {
  return { elements: [], revisions: [1, 1, 1], layout: 0, fields, metadata: null, flattened: false, can_undo: true, can_redo: false, dirty: true, ...extra };
}

const EMPTY = { ...editView(), revisions: [0, 0, 0], can_undo: false, dirty: false };

async function openForm(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/formulaire.pdf"] }],
    open_document: [{ ok: { ...documentView(3), file_name: "formulaire.pdf" } }],
    edit_view: [{ ok: EMPTY }],
    form_fields: [{ ok: FIELDS }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "formulaire.pdf" }).waitFor();
  await app.page.locator(".form-layer .form-field").first().waitFor();
  return app;
}

async function waitForCalls(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

const control = (app, name) => app.page.locator(`.form-layer [aria-label="${name}"]`);

test("Formulaire : un contrôle par champ en mode Lire, aucun en mode Éditer", async () => {
  const app = await openForm();
  try {
    assert.equal(await app.page.locator(".page").first().locator(".form-field").count(), 6);
    assert.equal(await control(app, "nom").inputValue(), "Ancien");
    assert.equal(await control(app, "ref").isDisabled(), true);
    assert.equal(await control(app, "accord").getAttribute("aria-checked"), "false");
    await app.page.click("#mode-edit");
    await app.page.locator(".form-layer").waitFor({ state: "detached" });
    await app.page.click("#mode-read");
    await control(app, "nom").waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Formulaire : texte, case, bouton radio et liste envoient leur valeur", async () => {
  const app = await openForm({
    set_field: [
      { ok: editView({ nom: { kind: "text", value: "Jean Dupont" } }) },
      { ok: editView({ accord: { kind: "checked", value: true } }) },
      { ok: editView({ civilite: { kind: "choice", value: "Mme" } }) },
      { ok: editView({ ville: { kind: "choice", value: "Lyon" } }) },
    ],
  });
  try {
    await control(app, "nom").fill("Jean Dupont");
    await control(app, "nom").press("Enter");
    await waitForCalls(app, "set_field", 1);
    await control(app, "accord").click();
    await waitForCalls(app, "set_field", 2);
    await app.page.locator('.form-layer [aria-label="civilite"]').nth(1).click();
    await waitForCalls(app, "set_field", 3);
    await control(app, "ville").selectOption("Lyon");
    const calls = await waitForCalls(app, "set_field", 4);
    assert.deepEqual(calls, [
      { id: 1, name: "nom", value: { kind: "text", value: "Jean Dupont" } },
      { id: 1, name: "accord", value: { kind: "checked", value: true } },
      { id: 1, name: "civilite", value: { kind: "choice", value: "Mme" } },
      { id: 1, name: "ville", value: { kind: "choice", value: "Lyon" } },
    ]);
    await app.page.locator("#doc-dirty:visible").waitFor();
  } finally {
    await app.close();
  }
});

test("Formulaire refusé par Rust : message, valeur d'origine remise", async () => {
  const app = await openForm({ set_field: [{ error: { kind: "invalid_request", message: "30 caractères au plus" } }] });
  try {
    await control(app, "nom").fill("trop long");
    await control(app, "nom").press("Enter");
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /Formulaire : 30 caractères au plus/);
    assert.equal(await control(app, "nom").inputValue(), "Ancien");
  } finally {
    await app.close();
  }
});

test("Aplatir le formulaire : les champs disparaissent, le menu se grise", async () => {
  const app = await openForm({ set_flatten: [{ ok: editView({}, { flattened: true }) }] });
  try {
    await app.page.click("#menu-button");
    assert.equal(await app.page.isDisabled("#menu-flatten"), false);
    await app.page.click("#menu-flatten");
    assert.deepEqual(await waitForCalls(app, "set_flatten"), [{ id: 1, flatten: true }]);
    await app.page.locator(".form-layer").waitFor({ state: "detached" });
    assert.equal(await app.page.isDisabled("#menu-flatten"), true);
  } finally {
    await app.close();
  }
});

test("Propriétés du document : lues, changées, appliquées", async () => {
  const original = { title: "Contrat", author: "Moi", subject: "", keywords: "" };
  const app = await openForm({
    document_metadata: [{ ok: original }],
    set_metadata: [{ ok: editView({}, { metadata: { ...original, title: "Contrat signé" } }) }],
  });
  try {
    await app.page.click("#menu-button");
    await app.page.click("#menu-properties");
    await app.page.locator("#props-dialog[open]").waitFor();
    assert.equal(await app.page.inputValue("#meta-title"), "Contrat");
    assert.equal(await app.page.inputValue("#meta-author"), "Moi");
    await app.page.fill("#meta-title", "  Contrat signé ");
    await app.page.fill("#meta-keywords", "contrat, été");
    await app.page.click('#props-form button[type="submit"]');
    const [call] = await waitForCalls(app, "set_metadata");
    assert.deepEqual(call, { id: 1, metadata: { title: "Contrat signé", author: "Moi", subject: "", keywords: "contrat, été" } });
    await app.page.locator("#props-dialog[open]").waitFor({ state: "detached" });
    await app.page.locator("#doc-dirty:visible").waitFor();
  } finally {
    await app.close();
  }
});
