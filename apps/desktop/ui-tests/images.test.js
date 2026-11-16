// Images (E-02) : bouton Image, coller, déposer, poignées d'angle,
// rotation, opacité, proportions.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const IMAGE = {
  kind: "image",
  page: 0,
  top_left: { x: 100, y: 200 },
  width: 200,
  height: 100,
  image: 1,
  rotation: 0,
  opacity: 1,
  keep_ratio: true,
};

function view(element = IMAGE, { dirty = true } = {}) {
  return {
    elements: element ? [{ id: 4, element, bounds: { ...element.top_left, width: element.width, height: element.height } }] : [],
    revisions: [dirty ? 1 : 0, 0, 0],
    can_undo: dirty,
    can_redo: false,
    dirty,
  };
}

const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };
const IMPORTED = { image_id: 1, width: 200, height: 100 };

async function openDocument(commands = {}, { edit = true } = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  if (edit) {
    await app.page.click("#mode-edit");
    await app.page.locator("#edit-toolbar:visible").waitFor();
  }
  return app;
}

async function waitForCall(app, command, count = 1) {
  await app.page.waitForFunction(
    ([name, n]) => window.__calls.filter((c) => c.command === name).length >= n,
    [command, count],
  );
  return app.callsTo(command);
}

async function selectImage(app) {
  await app.page.locator(".edit-layer .element").first().click();
  await app.page.locator("#prop-image-fields:visible").waitFor();
}

test("bouton Image : l'image choisie est posée au centre de la page, à sa taille naturelle", async () => {
  const app = await openDocument({ pick_image: [{ ok: IMPORTED }], add_element: [{ ok: { id: 4, view: view() } }] });
  try {
    await app.page.click("#tool-image");
    const [added] = await waitForCall(app, "add_element");
    assert.deepEqual(await app.callsTo("pick_image"), [{ id: 1 }]);
    assert.equal(added.element.kind, "image");
    assert.equal(added.element.image, 1);
    // 200 × 100 px à 96 ppp : 150 × 75 points.
    assert.deepEqual([added.element.width, added.element.height], [150, 75]);
    assert.equal(added.element.page, 0);
    assert.ok(added.element.top_left.x > 200 && added.element.top_left.x < 250, JSON.stringify(added.element));

    // Sélectionnée : panneau Image, quatre poignées d'angle.
    await app.page.locator("#prop-image-fields:visible").waitFor();
    assert.equal(await app.page.textContent("#properties-title"), "Image");
    assert.equal(await app.page.locator("#prop-text-fields").isVisible(), false);
    assert.equal(await app.page.locator(".element.selected .handle").count(), 4);
    assert.equal(await app.page.textContent("#prop-opacity-value"), "100 %");
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("bouton Image annulé : rien n'est ajouté", async () => {
  const app = await openDocument({ pick_image: [{ ok: null }] });
  try {
    await app.page.click("#tool-image");
    await waitForCall(app, "pick_image");
    assert.deepEqual(await app.callsTo("add_element"), []);
  } finally {
    await app.close();
  }
});

test("image refusée par Rust : message, rien n'est ajouté", async () => {
  const app = await openDocument({
    pick_image: [{ error: { kind: "invalid_file", message: "format d'image non pris en charge : PNG, JPEG ou WebP" } }],
  });
  try {
    await app.page.click("#tool-image");
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /Image non ajoutée.*PNG, JPEG ou WebP/);
    assert.deepEqual(await app.callsTo("add_element"), []);
  } finally {
    await app.close();
  }
});

test("Ctrl+V colle une image : octets envoyés sans JSON, passage en mode Éditer", async () => {
  const app = await openDocument(
    { import_image_bytes: [{ ok: IMPORTED }], add_element: [{ ok: { id: 4, view: view() } }] },
    { edit: false },
  );
  try {
    await app.page.evaluate(() => {
      const data = new DataTransfer();
      data.items.add(new File([new Uint8Array([137, 80, 78, 71, 1, 2, 3])], "image.png", { type: "image/png" }));
      document.dispatchEvent(new ClipboardEvent("paste", { clipboardData: data, bubbles: true }));
    });
    const [imported] = await waitForCall(app, "import_image_bytes");
    assert.deepEqual(imported, { bytes: 7, headers: { "x-document": "1" } });
    await waitForCall(app, "add_element");
    assert.equal(await app.page.locator("#mode-edit").getAttribute("aria-current"), "page");
  } finally {
    await app.close();
  }
});

test("image déposée sur la page : posée à l'endroit du dépôt", async () => {
  const app = await openDocument({ import_image_file: [{ ok: IMPORTED }], add_element: [{ ok: { id: 4, view: view() } }] });
  try {
    const box = await app.page.locator(".page").first().boundingBox();
    // Point au quart de la largeur et au tiers de la hauteur de la page 1.
    const x = box.x + box.width / 4;
    const y = box.y + box.height / 3;
    const ratio = await app.page.evaluate(() => window.devicePixelRatio);
    await app.emit("tauri://drag-drop", { paths: ["/photos/Logo.PNG"], position: { x: x * ratio, y: y * ratio } });
    const [added] = await waitForCall(app, "add_element");
    assert.deepEqual(await app.callsTo("import_image_file"), [{ id: 1, path: "/photos/Logo.PNG" }]);
    // Centre de l'image = point de dépôt (595 / 4, 842 / 3).
    const cx = added.element.top_left.x + added.element.width / 2;
    const cy = added.element.top_left.y + added.element.height / 2;
    assert.ok(Math.abs(cx - 595 / 4) < 2 && Math.abs(cy - 842 / 3) < 2, `${cx}, ${cy}`);
    assert.deepEqual(await app.callsTo("open_document"), [{ path: "/docs/contrat.pdf", password: null }]);
  } finally {
    await app.close();
  }
});

test("image déposée sans document ouvert : message", async () => {
  const app = await launch();
  try {
    await app.emit("tauri://drag-drop", { paths: ["/photos/logo.png"] });
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /Ouvrez d'abord un PDF/);
    assert.deepEqual(await app.callsTo("import_image_file"), []);
  } finally {
    await app.close();
  }
});

test("tirer un coin agrandit l'image en gardant ses proportions", async () => {
  const app = await openDocument({ edit_view: [{ ok: view() }], update_element: [{ ok: view() }] });
  try {
    await selectImage(app);
    const handle = await app.page.locator(".element.selected .handle.se").boundingBox();
    const layer = await app.page.locator(".page .edit-layer").first().boundingBox();
    const dx = (100 * layer.width) / 595; // 100 points vers la droite
    const start = { x: handle.x + handle.width / 2, y: handle.y + handle.height / 2 };
    await app.page.mouse.move(start.x, start.y);
    await app.page.mouse.down();
    await app.page.mouse.move(start.x + dx / 2, start.y + 5, { steps: 3 });
    await app.page.mouse.move(start.x + dx, start.y + 5, { steps: 3 });
    await app.page.mouse.up();
    const [update] = await waitForCall(app, "update_element");
    const e = update.element;
    assert.ok(Math.abs(e.width - 300) < 1.5, `largeur ${e.width}`);
    assert.ok(Math.abs(e.width / e.height - 2) < 1e-6, `proportions ${e.width / e.height}`);
    assert.deepEqual(e.top_left, IMAGE.top_left);
  } finally {
    await app.close();
  }
});

test("Propriétés : rotation, opacité, proportions et hauteur en mm", async () => {
  const app = await openDocument({ edit_view: [{ ok: view() }], update_element: [{ ok: view() }] });
  try {
    await selectImage(app);
    await app.page.click("#prop-rotate-right");
    // fill() sur un curseur envoie « input » puis « change ».
    await app.page.locator("#prop-opacity").fill("50");
    // Clic (la fausse réponse de Rust remet la case : seule la demande compte).
    await app.page.click("#prop-keep-ratio");
    assert.equal(await app.page.locator("#prop-height").getAttribute("readonly"), null);
    await app.page.fill("#prop-height", "50");
    await app.page.press("#prop-height", "Tab");
    const [rotation, opacity, ratio, height] = await waitForCall(app, "update_element", 4);
    assert.deepEqual([rotation.element.rotation, rotation.element.width, rotation.element.height], [1, 100, 200]);
    assert.equal(opacity.element.opacity, 0.5);
    assert.equal(ratio.element.keep_ratio, false);
    // Proportions gardées par l'élément reçu (vue inchangée) : la largeur suit.
    assert.ok(Math.abs(height.element.height - (50 * 72) / 25.4) < 0.01);
    assert.ok(Math.abs(height.element.width / height.element.height - 2) < 1e-6);
  } finally {
    await app.close();
  }
});

test("le style de texte ne s'applique pas à une image sélectionnée", async () => {
  const app = await openDocument({ edit_view: [{ ok: view() }], remove_element: [{ ok: view(null) }] });
  try {
    await selectImage(app);
    await app.page.click('#edit-toolbar [data-style="bold"]');
    assert.deepEqual(await app.callsTo("update_element"), []);
    // Mais Suppr la supprime, comme une zone de texte.
    await app.page.locator("#desk").focus();
    await app.page.keyboard.press("Delete");
    assert.deepEqual(await waitForCall(app, "remove_element"), [{ id: 1, elementId: 4 }]);
    await app.page.locator(".edit-layer .element").waitFor({ state: "detached" });
  } finally {
    await app.close();
  }
});
