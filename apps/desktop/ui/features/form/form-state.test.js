import assert from "node:assert/strict";
import { test } from "node:test";

import { blocked, clicked, currentValue, hint, isOn, textOf, typed, widgetsOn } from "./form-state.js";

const NAME = { name: "nom", kind: { kind: "text", multiline: false, max_len: null }, value: { kind: "text", value: "Ancien" }, widgets: [{ page: 0, option: null }] };
const OK = { name: "accord", kind: { kind: "checkbox" }, value: { kind: "checked", value: false }, widgets: [{ page: 0, option: null }] };
const CIVILITY = { name: "civilite", kind: { kind: "radio", options: ["M", "Mme"] }, value: null, widgets: [{ page: 0, option: 0 }, { page: 1, option: 1 }] };
const CITY = { name: "ville", kind: { kind: "choice", options: ["Paris", "Lyon"], editable: false }, value: null, widgets: [{ page: 1, option: null }] };

test("valeur saisie, sinon celle du document", () => {
  assert.equal(textOf(currentValue(NAME, {})), "Ancien");
  assert.equal(textOf(currentValue(NAME, { nom: { kind: "text", value: "Jean" } })), "Jean");
  assert.equal(textOf(currentValue(CITY, {})), "");
  assert.deepEqual(typed(NAME, "Paul"), { kind: "text", value: "Paul" });
  assert.deepEqual(typed(CITY, "Lyon"), { kind: "choice", value: "Lyon" });
});

test("case à cocher et boutons radio : état et clic", () => {
  const box = OK.widgets[0];
  assert.equal(isOn(OK, box, OK.value), false);
  assert.deepEqual(clicked(OK, box, OK.value), { kind: "checked", value: true });
  assert.deepEqual(clicked(OK, box, { kind: "checked", value: true }), { kind: "checked", value: false });
  const [m, mme] = CIVILITY.widgets;
  const value = { kind: "choice", value: "Mme" };
  assert.equal(isOn(CIVILITY, m, value), false);
  assert.equal(isOn(CIVILITY, mme, value), true);
  assert.deepEqual(clicked(CIVILITY, m, value), { kind: "choice", value: "M" });
  assert.equal(isOn(NAME, NAME.widgets[0], null), false);
});

test("emplacements d'une page", () => {
  const all = [NAME, OK, CIVILITY, CITY];
  assert.deepEqual(widgetsOn(all, 0).map((w) => w.field.name), ["nom", "accord", "civilite"]);
  assert.deepEqual(widgetsOn(all, 1).map((w) => w.field.name), ["civilite", "ville"]);
  assert.deepEqual(widgetsOn(all, 2), []);
});

test("champs intelligents : exemple de saisie, champs non modifiables, signatures écartées", () => {
  const text = (extra) => ({ name: "x", read_only: false, kind: { kind: "text", multiline: false, max_len: null, ...extra }, widgets: [{ page: 0 }] });
  assert.equal(hint(text({ format: { kind: "date", pattern: "dd/mm/yyyy" } })), "jj/mm/aaaa");
  assert.equal(hint(text({ format: { kind: "number", decimals: 2, separator: "comma", currency: " €", currency_before: false } })), "1234,50 €");
  assert.equal(hint(text({ format: { kind: "mask", mask: "99 99 99 99 99" } })), "00 00 00 00 00");
  assert.equal(hint(text({ format: null })), "");
  assert.equal(hint(OK), "");
  assert.equal(blocked(text({ calc: { op: "sum", fields: ["a"] } })), "calculated");
  assert.equal(blocked(text({ password: true })), "password");
  assert.equal(blocked({ ...NAME, read_only: true }), "readOnly");
  assert.equal(blocked(NAME), null);
  const signature = { name: "sig", kind: { kind: "signature" }, widgets: [{ page: 0 }] };
  assert.deepEqual(widgetsOn([signature, NAME], 0).map((w) => w.field.name), ["nom"]);
  assert.equal(textOf({ kind: "choices", value: ["fr", "de"] }), "fr");
});
