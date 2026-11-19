// Mode Formulaire (E-12) : cadres des champs, réglages, création d'un
// champ intelligent, détection des champs, suppression ; et le remplissage
// des champs intelligents en mode Lire.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const look = { font_size: 0, align: "left", color: { r: 0, g: 0, b: 0 }, border: { r: 128, g: 138, b: 150 }, background: null };
const widget = (x, y, extra = {}) => ({ page: 0, option: null, index: 0, export: null, element: null, bounds: { x, y, width: 160, height: 20 }, ...extra });
const field = (name, kind, extra = {}) => ({ name, kind, read_only: false, required: false, tooltip: "", look, value: null, created: false, widgets: [widget(120, 120)], ...extra });
const text = (extra = {}) => ({ kind: "text", multiline: false, max_len: null, password: false, comb: false, format: null, calc: null, ...extra });

const NAME = field("nom", text());
const DATE = field("date", text({ format: { kind: "date", pattern: "dd/mm/yyyy" } }), { required: true, widgets: [widget(120, 180)] });
const TOTAL = field("total", text({ calc: { op: "sum", fields: ["a", "b"] } }), { widgets: [widget(120, 240)] });
const LANGS = field("langues", { kind: "choice", options: ["fr", "en", "de"], editable: false, list: true, multi: true }, { widgets: [widget(120, 300, { bounds: { x: 120, y: 300, width: 100, height: 50 } })] });
const COUNTRY = field("pays", { kind: "choice", options: ["France", "Belgique"], editable: true, list: false, multi: false }, { widgets: [widget(300, 120)] });
const SIGNATURE = field("signature", { kind: "signature" }, { widgets: [widget(300, 400, { bounds: { x: 300, y: 400, width: 180, height: 50 } })] });
const FIELDS = [NAME, DATE, TOTAL, LANGS, COUNTRY, SIGNATURE];

function editView(extra = {}) {
  return { elements: [], revisions: [1, 1, 1], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: true, can_redo: false, dirty: true, ...extra };
}

const EMPTY = { ...editView(), revisions: [0, 0, 0], can_undo: false, dirty: false };
/** État après une conception : un élément « field » change l'empreinte, les champs sont relus. */
/** Élément de conception tel que Rust l'envoie (`kind` : l'élément, `field_kind` : le genre du champ). */
const fieldElement = (id) => ({
  id,
  element: {
    kind: "field",
    page: 0,
    top_left: { x: 200, y: 500 },
    width: 100,
    height: 20,
    name: "date_1",
    field_kind: text({ format: { kind: "date", pattern: "dd/mm/yyyy" } }),
    tooltip: "",
    required: false,
    read_only: false,
    look,
    value: null,
    export: "",
    origin: null,
    deleted: false,
  },
  bounds: { x: 200, y: 500, width: 100, height: 20 },
  missing_chars: "",
  replaced_font: false,
});
const designed = (id) => editView({ elements: [fieldElement(id)] });

async function openForm(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/formulaire.pdf"] }],
    open_document: [{ ok: { ...documentView(3), file_name: "formulaire.pdf" } }],
    edit_view: [{ ok: EMPTY }],
    form_fields: [{ ok: FIELDS }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "formulaire.pdf" }).waitFor();
  return app;
}

async function waitForCalls(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

async function formMode(app) {
  await app.page.click("#mode-form");
  await app.page.locator("#form-toolbar:visible").waitFor();
  await app.page.locator(".page").first().locator(".design-box").first().waitFor();
}

const designBox = (app, name) => app.page.locator(".design-box", { has: app.page.locator(".design-label", { hasText: new RegExp(`^${name}$`) }) });

test("Mode Formulaire : un cadre par champ, réglages d'un champ renvoyés à Rust", async () => {
  const app = await openForm({ set_form_field: [{ ok: designed(1) }] });
  try {
    await formMode(app);
    assert.equal(await app.page.locator(".page").first().locator(".design-box").count(), 6);
    assert.equal(await app.page.locator(".design-box.signature").count(), 1);
    assert.equal(await app.page.isVisible("#form-empty"), true);
    await designBox(app, "date").click();
    await app.page.locator("#form-props:visible").waitFor();
    assert.equal(await app.page.inputValue("#form-name"), "date");
    assert.equal(await app.page.inputValue("#form-format"), "date");
    assert.equal(await app.page.isChecked("#form-required"), true);
    await app.page.fill("#form-tooltip", "Date de naissance");
    await app.page.press("#form-tooltip", "Tab");
    const [call] = await waitForCalls(app, "set_form_field");
    assert.equal(call.name, "date");
    assert.equal(call.field.tooltip, "Date de naissance");
    assert.equal(call.field.required, true);
    assert.deepEqual(call.field.field_kind.format, { kind: "date", pattern: "dd/mm/yyyy" });
    // Changement de format : un montant.
    await app.page.selectOption("#form-format", "amount");
    const calls = await waitForCalls(app, "set_form_field", 2);
    assert.deepEqual(calls[1].field.field_kind.format, { kind: "number", decimals: 2, separator: "comma", currency: " €", currency_before: false });
    // L'édition a changé : champs relus.
    await waitForCalls(app, "form_fields", 2);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Mode Formulaire : outil Date, un clic crée un champ date", async () => {
  const created = field("date_1", text({ format: { kind: "date", pattern: "dd/mm/yyyy" } }), { created: true, widgets: [widget(200, 500, { element: 7 })] });
  const app = await openForm({
    add_form_fields: [{ ok: { ids: [7], view: designed(7) } }],
    // Lus par le mode Lire et le mode Formulaire, puis relus après la création.
    form_fields: [{ ok: FIELDS }, { ok: FIELDS }, { ok: [...FIELDS, created] }],
  });
  try {
    await formMode(app);
    await app.page.click('[data-field-tool="date"]');
    assert.equal(await app.page.getAttribute('[data-field-tool="date"]', "aria-pressed"), "true");
    const layer = app.page.locator(".page").first().locator(".design-layer.drawing");
    const box = await layer.boundingBox();
    await app.page.mouse.click(box.x + box.width * 0.6, box.y + 40);
    const [call] = await waitForCalls(app, "add_form_fields");
    assert.equal(call.fields.length, 1);
    const [f] = call.fields;
    assert.equal(f.name, "date_1");
    assert.equal(f.page, 0);
    assert.deepEqual(f.field_kind.format, { kind: "date", pattern: "dd/mm/yyyy" });
    assert.equal(f.width, 100);
    // Le champ créé est choisi, l'outil revient à « Choisir ».
    await app.page.locator("#form-name").waitFor();
    await app.page.waitForFunction(() => document.getElementById("form-name").value === "date_1");
    assert.equal(await app.page.getAttribute('[data-field-tool="select"]', "aria-pressed"), "true");
    // Le champ créé n'est pas un ajout du mode Éditer : rien ne casse en y passant.
    await app.page.click("#mode-edit");
    await app.page.click("#mode-form");
    assert.equal(await app.page.locator(".edit-layer [data-id]").count(), 0);
    assert.equal(await app.page.locator("#toast.error").count(), 0);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Mode Formulaire : champs détectés proposés, ajoutés seulement s'ils sont cochés", async () => {
  const suggestion = (name, y) => ({
    page: 0,
    top_left: { x: 100, y },
    width: 150,
    height: 18,
    name,
    field_kind: text(),
    tooltip: name,
    required: false,
    read_only: false,
    look,
    value: null,
    export: "",
    origin: null,
    deleted: false,
  });
  const app = await openForm({
    suggest_form_fields: [{ ok: [suggestion("nom_client", 600), suggestion("ville", 640)] }],
    add_form_fields: [{ ok: { ids: [3], view: designed(3) } }],
  });
  try {
    await formMode(app);
    await app.page.click("#form-detect");
    await app.page.locator("#form-suggestions:visible").waitFor();
    assert.equal(await app.page.locator(".design-suggestion.chosen").count(), 2);
    assert.deepEqual(await waitForCalls(app, "suggest_form_fields"), [{ id: 1, page: 0 }]);
    await app.page.locator("#form-suggestions-list input").nth(1).uncheck();
    assert.equal(await app.page.locator(".design-suggestion.chosen").count(), 1);
    await app.page.click("#form-suggestions-add");
    const [call] = await waitForCalls(app, "add_form_fields");
    assert.deepEqual(call.fields.map((f) => f.name), ["nom_client"]);
    await app.page.locator("#form-suggestions").waitFor({ state: "hidden" });
  } finally {
    await app.close();
  }
});

test("Mode Formulaire : Suppr supprime le champ choisi", async () => {
  const app = await openForm({ delete_form_widgets: [{ ok: designed(2) }] });
  try {
    await formMode(app);
    await designBox(app, "nom").click();
    await app.page.keyboard.press("Delete");
    assert.deepEqual(await waitForCalls(app, "delete_form_widgets"), [{ id: 1, targets: [{ name: "nom", index: null }] }]);
  } finally {
    await app.close();
  }
});

test("Mode Lire : date au calendrier, obligatoire signalé, total calculé, listes", async () => {
  const app = await openForm({
    set_field: [
      { ok: editView({ fields: { date: { kind: "text", value: "19/11/2026" } } }) },
      { ok: editView({ fields: { langues: { kind: "choices", value: ["fr", "de"] } } }) },
    ],
  });
  try {
    const control = (name) => app.page.locator(`.form-layer [aria-label="${name}"]`);
    await control("nom").waitFor();
    // La signature se signe en mode Signer : pas de contrôle ici.
    assert.equal(await control("signature").count(), 0);
    assert.equal(await control("date").getAttribute("aria-required"), "true");
    assert.equal(await control("date").getAttribute("placeholder"), "jj/mm/aaaa");
    assert.equal(await control("total").isDisabled(), true);
    assert.match(await control("total").getAttribute("title"), /calculé/);
    assert.equal(await app.page.locator(`.form-layer datalist option`).count(), 2);
    assert.equal(await app.page.locator(".form-layer .form-date").count(), 1);
    await app.page.locator(".form-layer .form-date-input").evaluate((input) => {
      input.value = "2026-11-19";
      input.dispatchEvent(new Event("change"));
    });
    const [first] = await waitForCalls(app, "set_field");
    assert.deepEqual(first, { id: 1, name: "date", value: { kind: "text", value: "2026-11-19" } });
    await control("langues").selectOption(["fr", "de"]);
    const calls = await waitForCalls(app, "set_field", 2);
    assert.deepEqual(calls[1], { id: 1, name: "langues", value: { kind: "choices", value: ["fr", "de"] } });
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Mode Formulaire : « Une case par caractère » règle des cases lisibles (bogue #57)", async () => {
  const app = await openForm({ set_form_field: [{ ok: designed(1) }] });
  try {
    await formMode(app);
    await designBox(app, "nom").click();
    await app.page.locator("#form-props:visible").waitFor();
    await app.page.fill("#form-max-len", "100");
    await app.page.press("#form-max-len", "Tab");
    await waitForCalls(app, "set_form_field", 1);
    // Le faux Rust relit les mêmes champs : on attend ce rechargement avant de cocher.
    await waitForCalls(app, "form_fields", 3);
    await app.page.click("#form-comb");
    const calls = await waitForCalls(app, "set_form_field", 2);
    // Champ de 160 × 20 pt : 100 cases (1,6 pt) ramenées à 10 cases de 16 pt.
    assert.equal(calls[1].field.field_kind.comb, true);
    assert.equal(calls[1].field.field_kind.max_len, 10);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});
