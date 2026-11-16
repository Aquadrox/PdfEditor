// Mode Éditer : zones de texte, propriétés, annuler / rétablir,
// enregistrement et question avant de perdre des modifications.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const STYLE = {
  font: "sans",
  size: 11,
  bold: false,
  italic: false,
  color: { r: 0x1c, g: 0x24, b: 0x30 },
  align: "left",
  line_height: 1.25,
};

const BOX = { kind: "text", page: 0, top_left: { x: 100, y: 200 }, width: 200, text: "Bonjour", style: STYLE };

/** EditView de pde-app avec une zone (ou aucune). */
function view({ element = BOX, dirty = true, canUndo = dirty, canRedo = false, revision = dirty ? 1 : 0 } = {}) {
  return {
    elements: element ? [{ id: 7, element, bounds: { ...element.top_left, width: element.width, height: 14 } }] : [],
    revisions: [revision, 0, 0],
    can_undo: canUndo,
    can_redo: canRedo,
    dirty,
  };
}

const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };

async function openInEditMode(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator("#edit-toolbar:visible").waitFor();
  return app;
}

/** Zone existante, sélectionnée d'un clic. */
async function selectBox(app) {
  await app.page.locator(".edit-layer .element").first().waitFor();
  await app.page.locator(".edit-layer .element").first().click();
  await app.page.locator("#prop-fields:visible").waitFor();
}

test("Éditer : un clic sur la page puis la saisie ajoute une zone de texte", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 7, view: view() } }] });
  try {
    // Adresses des images demandées (le faux protocole ne les sert pas).
    await app.page.evaluate(() => {
      window.__images = [];
      const src = Object.getOwnPropertyDescriptor(HTMLImageElement.prototype, "src");
      Object.defineProperty(HTMLImageElement.prototype, "src", {
        get() {
          return src.get.call(this);
        },
        set(value) {
          window.__images.push(value);
          src.set.call(this, value);
        },
      });
    });
    // Panneau ouvert dès l'entrée dans le mode : la page ne change pas de
    // zoom à la première sélection.
    assert.equal(await app.page.locator("#prop-empty").isVisible(), true);
    const zoom = await app.page.textContent("#zoom-value");
    const layer = app.page.locator(".page").first().locator(".edit-layer");
    const box = await layer.boundingBox();
    // Clic au quart de la largeur et au cinquième de la hauteur de la page.
    await app.page.mouse.click(box.x + box.width / 4, box.y + box.height / 5);
    await app.page.keyboard.type("Bonjour");
    await app.page.keyboard.press("Control+Enter");
    await app.page.locator(".edit-layer .element.selected").waitFor();

    const [added] = await app.callsTo("add_element");
    assert.equal(added.id, 1);
    assert.equal(added.element.kind, "text");
    assert.equal(added.element.page, 0);
    assert.equal(added.element.text, "Bonjour");
    assert.deepEqual(added.element.style, STYLE);
    assert.ok(Math.abs(added.element.top_left.x - 595 / 4) < 2, `x = ${added.element.top_left.x}`);
    assert.ok(Math.abs(added.element.top_left.y - 842 / 5) < 2, `y = ${added.element.top_left.y}`);

    // Panneau, pastille « modifié », nouvelle image de la page modifiée.
    assert.equal(await app.page.inputValue("#prop-text"), "Bonjour");
    assert.equal(await app.page.textContent("#zoom-value"), zoom);
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), true);
    assert.equal(await app.page.textContent("#save-state"), "Modifications non enregistrées");
    await app.page.waitForFunction(() => window.__images.some((url) => /page\/1\/0\?.*rev=1/.test(url)));
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Échap annule la saisie : rien n'est ajouté", async () => {
  const app = await openInEditMode();
  try {
    const box = await app.page.locator(".page .edit-layer").first().boundingBox();
    await app.page.mouse.click(box.x + 100, box.y + 100);
    await app.page.keyboard.type("Brouillon");
    await app.page.keyboard.press("Escape");
    assert.equal(await app.page.locator(".text-editor").count(), 0);
    assert.deepEqual(await app.callsTo("add_element"), []);
  } finally {
    await app.close();
  }
});

test("déplacer une zone à la souris envoie sa nouvelle position", async () => {
  const moved = { ...BOX, top_left: { x: 150, y: 200 } };
  const app = await openInEditMode({
    edit_view: [{ ok: view({ dirty: false }) }],
    update_element: [{ ok: view({ element: moved, revision: 2 }) }],
  });
  try {
    await selectBox(app);
    const element = app.page.locator(".edit-layer .element").first();
    const start = await element.boundingBox();
    const layer = await app.page.locator(".page .edit-layer").first().boundingBox();
    const dx = (50 * layer.width) / 595; // 50 points à l'écran
    await app.page.mouse.move(start.x + 20, start.y + 5);
    await app.page.mouse.down();
    await app.page.mouse.move(start.x + 20 + dx / 2, start.y + 5, { steps: 3 });
    await app.page.mouse.move(start.x + 20 + dx, start.y + 5, { steps: 3 });
    await app.page.mouse.up();
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "update_element"));

    const [update] = await app.callsTo("update_element");
    assert.equal(update.elementId, 7);
    assert.ok(Math.abs(update.element.top_left.x - 150) < 1, `x = ${update.element.top_left.x}`);
    assert.ok(Math.abs(update.element.top_left.y - 200) < 1, `y = ${update.element.top_left.y}`);
  } finally {
    await app.close();
  }
});

test("Propriétés : couleur, alignement et position en mm", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: view({ dirty: false }) }], update_element: [{ ok: view() }] });
  try {
    await selectBox(app);
    assert.equal(await app.page.inputValue("#prop-x"), "35,3");
    await app.page.click('#prop-swatches [data-value="#8a1c2b"]');
    await app.page.click('[data-style="align"][data-value="center"]');
    await app.page.fill("#prop-x", "50");
    await app.page.press("#prop-x", "Tab");
    await app.page.waitForFunction(() => window.__calls.filter((c) => c.command === "update_element").length === 3);

    const [color, align, position] = await app.callsTo("update_element");
    assert.deepEqual(color.element.style.color, { r: 0x8a, g: 0x1c, b: 0x2b });
    assert.equal(align.element.style.align, "center");
    assert.ok(Math.abs(position.element.top_left.x - (50 * 72) / 25.4) < 0.01);
  } finally {
    await app.close();
  }
});

test("Suppr supprime la zone sélectionnée", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view({ dirty: false }) }],
    remove_element: [{ ok: view({ element: null }) }],
  });
  try {
    await selectBox(app);
    await app.page.locator("#desk").focus();
    await app.page.keyboard.press("Delete");
    await app.page.locator("#prop-fields").waitFor({ state: "hidden" });
    assert.deepEqual(await app.callsTo("remove_element"), [{ id: 1, elementId: 7 }]);
    assert.equal(await app.page.locator(".edit-layer .element").count(), 0);
  } finally {
    await app.close();
  }
});

test("Ctrl+Z et Ctrl+Y annulent et rétablissent, sauf dans un champ de saisie", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view() }],
    undo_edit: [{ ok: view({ element: null, dirty: false, canUndo: false, canRedo: true }) }],
    redo_edit: [{ ok: view() }],
  });
  try {
    await app.page.locator("#edit-undo:enabled").waitFor();
    await app.page.locator("#desk").focus();
    await app.page.keyboard.press("Control+z");
    await app.page.locator("#edit-redo:enabled").waitFor();
    assert.equal(await app.page.locator("#edit-undo").isDisabled(), true);
    await app.page.keyboard.press("Control+y");
    await app.page.locator("#edit-undo:enabled").waitFor();
    assert.equal((await app.callsTo("undo_edit")).length, 1);
    assert.equal((await app.callsTo("redo_edit")).length, 1);

    // Dans le texte du panneau, Ctrl+Z reste l'annulation de la saisie.
    await selectBox(app);
    await app.page.focus("#prop-text");
    await app.page.keyboard.press("Control+z");
    assert.equal((await app.callsTo("undo_edit")).length, 1);
  } finally {
    await app.close();
  }
});

test("Ctrl+S enregistre et la pastille « modifié » disparaît", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view() }, { ok: view({ dirty: false, canUndo: true }) }],
    save_document: [{ ok: { path: "/docs/contrat.pdf", file_name: "contrat.pdf" } }],
  });
  try {
    await app.page.locator("#doc-dirty:visible").waitFor();
    await app.page.keyboard.press("Control+s");
    await app.page.locator("#doc-dirty").waitFor({ state: "hidden" });
    assert.deepEqual(await app.callsTo("save_document"), [{ id: 1 }]);
    assert.equal(await app.page.textContent("#save-state"), "Enregistré");
    assert.match(await app.page.textContent("#toast"), /contrat\.pdf/);
  } finally {
    await app.close();
  }
});

test("Enregistrer sous propose le nom du fichier et affiche le nouveau nom", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view() }, { ok: view({ dirty: false }) }],
    save_document_as: [{ ok: { path: "/docs/copie.pdf", file_name: "copie.pdf" } }],
  });
  try {
    await app.page.keyboard.press("Control+Shift+S");
    await app.page.locator("#doc-name", { hasText: "copie.pdf" }).waitFor();
    assert.deepEqual(await app.callsTo("save_document_as"), [{ id: 1, suggestedName: "contrat.pdf" }]);
  } finally {
    await app.close();
  }
});

test("un échec d'enregistrement est signalé et le document reste modifié", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view() }],
    save_document: [{ error: { kind: "io", message: "disque plein" } }],
  });
  try {
    await app.page.keyboard.press("Control+s");
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /disque plein/);
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), true);
  } finally {
    await app.close();
  }
});

test("fermer un document modifié demande confirmation", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: view() }] });
  try {
    await app.page.locator("#doc-dirty:visible").waitFor();
    await app.page.keyboard.press("Control+w");
    await app.page.locator("#unsaved-dialog[open]").waitFor();
    assert.match(await app.page.textContent("#unsaved-text"), /contrat\.pdf/);

    // Annuler : le document reste ouvert.
    await app.page.click("#unsaved-cancel");
    await app.page.locator("#unsaved-dialog").waitFor({ state: "hidden" });
    assert.deepEqual(await app.callsTo("close_document"), []);

    // Ne pas enregistrer : fermé sans enregistrer.
    await app.page.keyboard.press("Control+w");
    await app.page.click("#unsaved-discard");
    await app.page.locator("#home:visible").waitFor();
    assert.deepEqual(await app.callsTo("close_document"), [{ id: 1 }]);
    assert.deepEqual(await app.callsTo("save_document"), []);
  } finally {
    await app.close();
  }
});

test("fermer la fenêtre avec des modifications : Enregistrer, puis quitter", async () => {
  const app = await openInEditMode({
    edit_view: [{ ok: view() }, { ok: view({ dirty: false }) }],
    save_document: [{ ok: { path: "/docs/contrat.pdf", file_name: "contrat.pdf" } }],
  });
  try {
    await app.page.locator("#doc-dirty:visible").waitFor();
    await app.emit("close-requested");
    await app.page.locator("#unsaved-dialog[open]").waitFor();
    await app.page.click("#unsaved-save");
    await app.page.waitForFunction(() => window.__calls.some((c) => c.command === "quit_app"));
    assert.deepEqual(await app.callsTo("save_document"), [{ id: 1 }]);
  } finally {
    await app.close();
  }
});

test("mode Lire : la couche d'édition disparaît, le texte redevient sélectionnable", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: view({ dirty: false }) }] });
  try {
    await app.page.locator(".edit-layer .element").first().waitFor();
    await app.page.click("#mode-read");
    await app.page.locator("#edit-toolbar").waitFor({ state: "hidden" });
    assert.equal(await app.page.locator(".edit-layer .element").first().isVisible(), false);
    assert.equal(await app.page.locator("#mode-read").getAttribute("aria-current"), "page");
  } finally {
    await app.close();
  }
});
