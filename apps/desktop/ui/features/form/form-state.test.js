import assert from "node:assert/strict";
import { test } from "node:test";

import { clicked, currentValue, isOn, textOf, typed, widgetsOn } from "./form-state.js";

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
