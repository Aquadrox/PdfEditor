import assert from "node:assert/strict";
import { test } from "node:test";

import { blockElement, blockItems, editorBox, isRemovedText, oneLine, withNewText } from "./original-text.js";

const STYLE = { font: "serif", size: 24, bold: false, italic: false, color: { r: 0, g: 0, b: 0 }, align: "left", line_height: 1.25 };
const BLOCK = { objects: [0, 1], bounds: { x: 72, y: 62, width: 218, height: 25 }, text: "Bonjour PdfEditor", font_name: "Helvetica", style: STYLE };

test("bloc proposé à la modification, avec sa place et son style", () => {
  const [item] = blockItems(0, [BLOCK], []);
  assert.equal(item.id, "otext:0:0");
  assert.equal(item.fontName, "Helvetica");
  assert.deepEqual(item.element.original, { top_left: { x: 72, y: 62 }, width: 218, height: 25 });
  assert.equal(item.element.before, "Bonjour PdfEditor");
  assert.equal(item.element.text, "Bonjour PdfEditor");
  assert.deepEqual(item.element.objects, [0, 1]);
});

test("un bloc déjà modifié n'est plus proposé, sa modification le représente", () => {
  const edited = { id: 4, element: { ...blockElement(0, BLOCK), text: "Bonsoir" } };
  assert.deepEqual(blockItems(0, [BLOCK, { ...BLOCK, objects: [5] }], [edited]).map((i) => i.id), ["otext:0:5"]);
  assert.equal(blockItems(1, [BLOCK], [edited]).length, 1);
});

test("texte saisi sur une ligne ; vide, le bloc est supprimé", () => {
  assert.equal(oneLine("deux\n lignes\r\nici"), "deux lignes ici");
  const element = withNewText(blockElement(0, BLOCK), "Bonsoir\nà tous");
  assert.equal(element.text, "Bonsoir à tous");
  assert.equal(element.before, "Bonjour PdfEditor");
  assert.ok(isRemovedText(withNewText(element, "  ")));
  assert.ok(!isRemovedText(element) && !isRemovedText({ kind: "text", text: "" }));
});

test("zone de saisie à la place du bloc, plus large sans sortir de la page", () => {
  const box = editorBox(blockElement(0, BLOCK), { width: 595, height: 842 });
  assert.deepEqual(box.top_left, { x: 72, y: 62 });
  assert.equal(box.width, 218 + 24 * 6);
  assert.equal(box.style.font, "serif");
  assert.equal(editorBox(blockElement(0, BLOCK), { width: 300, height: 842 }).width, 228);
});
