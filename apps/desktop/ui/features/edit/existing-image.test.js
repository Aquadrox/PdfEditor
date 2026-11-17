import assert from "node:assert/strict";
import { test } from "node:test";

import { asImage, fittedImageBox, isSynthetic, syntheticItem, syntheticItems, withFrame, withoutImage } from "./existing-image.js";

const IMAGE = { object: 3, bounds: { x: 100, y: 142, width: 200, height: 100 } };

test("image d'origine proposée à la sélection, avec son cadre", () => {
  const item = syntheticItem(0, IMAGE);
  assert.equal(item.id, "orig:0:3");
  assert.ok(isSynthetic(item.id) && !isSynthetic(7));
  assert.deepEqual(item.element.original, { top_left: { x: 100, y: 142 }, width: 200, height: 100 });
  assert.deepEqual(item.element.frame, item.element.original);
});

test("une image déjà modifiée n'est plus proposée, sa modification la représente", () => {
  const edited = { id: 9, element: { kind: "existing_image", page: 0, object: 3 } };
  assert.deepEqual(syntheticItems(0, [IMAGE, { ...IMAGE, object: 5 }], [edited]).map((i) => i.id), ["orig:0:5"]);
  // Même rang sur une autre page : sans rapport.
  assert.equal(syntheticItems(1, [IMAGE], [edited]).length, 1);
});

test("le cadre passe par les calculs d'une image ajoutée et revient", () => {
  const element = syntheticItem(0, IMAGE).element;
  const image = asImage(element);
  assert.deepEqual([image.kind, image.keep_ratio, image.width], ["image", true, 200]);
  const moved = withFrame(element, { ...image, top_left: { x: 10, y: 20 } });
  assert.deepEqual(moved.frame.top_left, { x: 10, y: 20 });
  assert.deepEqual(moved.original, element.original, "l'origine ne bouge pas");
  assert.equal(withoutImage(moved).frame, null);
  assert.deepEqual(asImage(withoutImage(moved)).top_left, { x: 100, y: 142 }, "supprimée : cadre d'origine");
});

test("une image de remplacement tient dans le cadre, centrée, à ses proportions", () => {
  const element = syntheticItem(2, IMAGE).element;
  // Image carrée dans un cadre 200 × 100 : 100 × 100, centrée en largeur.
  const box = fittedImageBox(element, { image_id: 4, width: 50, height: 50 });
  assert.deepEqual([box.kind, box.page, box.image], ["image", 2, 4]);
  assert.deepEqual([box.width, box.height], [100, 100]);
  assert.deepEqual(box.top_left, { x: 150, y: 142 });
});
