import assert from "node:assert/strict";
import { test } from "node:test";

import {
  boxAt,
  boxBetween,
  clampBox,
  displayValue,
  formatChoice,
  formatFor,
  FORMATS,
  kindKey,
  newField,
  parseOptions,
  templateOf,
  uniqueName,
} from "./design-state.js";

const A4 = { width: 595, height: 842 };

test("noms libres et nouveaux champs", () => {
  assert.equal(uniqueName("texte", []), "texte_1");
  assert.equal(uniqueName("texte", ["texte_1", "texte_2"]), "texte_3");
  const date = newField("date", { x: 10, y: 20, width: 100, height: 20 }, 1, ["date_1"]);
  assert.equal(date.name, "date_2");
  assert.equal(date.page, 1);
  assert.deepEqual(date.top_left, { x: 10, y: 20 });
  assert.deepEqual(date.kind.format, { kind: "date", pattern: "dd/mm/yyyy" });
  assert.equal(newField("checkbox", { x: 0, y: 0, width: 14, height: 14 }, 0, []).export, "Oui");
  const joined = newField("radio", { x: 0, y: 0, width: 14, height: 14 }, 0, [], { name: "civilite", exports: ["Choix1"] });
  assert.equal(joined.name, "civilite");
  assert.equal(joined.export, "Choix2");
});

test("boîtes tracées, cliquées et bornées à la page", () => {
  assert.equal(boxBetween({ x: 10, y: 10 }, { x: 12, y: 12 }, A4), null);
  assert.deepEqual(boxBetween({ x: 100, y: 50 }, { x: 20, y: 80 }, A4), { x: 20, y: 50, width: 80, height: 30 });
  assert.deepEqual(boxAt({ x: 580, y: 5 }, "text", A4), { x: 435, y: 0, width: 160, height: 20 });
  assert.deepEqual(clampBox({ x: -5, y: 900, width: 1, height: 10 }, A4), { x: 0, y: 832, width: 4, height: 10 });
});

test("formats : choix de la liste, réglages gardés", () => {
  assert.equal(formatChoice(null), "none");
  assert.equal(formatChoice(FORMATS.amount()), "amount");
  assert.equal(formatChoice(FORMATS.number()), "number");
  assert.equal(formatChoice(FORMATS.phone()), "phone");
  assert.equal(formatChoice({ kind: "mask", mask: "AA99" }), "mask");
  const custom = { kind: "date", pattern: "yyyy-mm-dd" };
  assert.deepEqual(formatFor("date", custom), custom);
  assert.deepEqual(formatFor("amount", FORMATS.number()), FORMATS.amount());
  assert.deepEqual(formatFor("phone", { kind: "mask", mask: "AA" }), FORMATS.phone());
  assert.equal(formatFor("none", custom), null);
});

test("montants et pourcentages affichés comme dans Rust", () => {
  assert.equal(displayValue(FORMATS.amount(), "1234.5"), "1234,50 €");
  const dollars = { kind: "number", decimals: 2, separator: "comma_dot", currency: "$", currency_before: true };
  assert.equal(displayValue(dollars, "1234567.891"), "$1,234,567.89");
  assert.equal(displayValue({ ...dollars, separator: "dot_comma", currency: "" }, "-1234"), "-1.234,00");
  assert.equal(displayValue({ kind: "percent", decimals: 1, separator: "comma" }, "0.155"), "15,5%");
  assert.equal(displayValue(FORMATS.amount(), "abc"), "abc");
  assert.equal(displayValue(FORMATS.date(), "19/11/2026"), "19/11/2026");
  assert.equal(displayValue(null, null), "");
});

test("réglages renvoyés à Rust et options", () => {
  const field = {
    name: "nom",
    kind: { kind: "text" },
    tooltip: "Nom",
    required: true,
    read_only: false,
    look: null,
    value: { kind: "text", value: "x" },
    created: false,
    widgets: [{ page: 2, bounds: { x: 1, y: 2, width: 3, height: 50 }, export: null }],
  };
  const template = templateOf(field, { name: "nom_complet" });
  assert.equal(template.name, "nom_complet");
  assert.equal(template.page, 2);
  assert.equal(template.width, 4);
  assert.equal(template.value, null);
  assert.equal(template.required, true);
  assert.deepEqual(parseOptions(" Paris\n\nLyon\nParis \n"), ["Paris", "Lyon"]);
  assert.equal(kindKey({ kind: { kind: "text", format: FORMATS.email() } }), "formDesign.format.email");
  assert.equal(kindKey({ kind: { kind: "choice", list: true } }), "formDesign.tool.list");
  assert.equal(kindKey({ kind: { kind: "signature" } }), "formDesign.tool.signature");
});
