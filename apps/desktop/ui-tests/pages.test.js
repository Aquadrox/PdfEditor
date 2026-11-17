// Mode Pages : grille, sélection, page blanche, dupliquer, pivoter,
// supprimer, glisser pour réordonner, annuler (document décrit à nouveau).

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

/** EditView de pde-app pour `count` pages. */
function editView(count, { layout = 0, revision = 0, dirty = false, canUndo = dirty } = {}) {
  return {
    elements: [],
    revisions: Array(count).fill(revision),
    layout,
    can_undo: canUndo,
    can_redo: false,
    dirty,
  };
}

/** Réponse d'une opération sur les pages (PagesView). */
function pagesView(count, selected, layout = 1) {
  return {
    document: documentView(count),
    edit: editView(count, { layout, revision: layout, dirty: true }),
    selected,
  };
}

async function openInPagesMode(commands = {}, count = 3) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(count) }],
    edit_view: [{ ok: editView(count) }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-pages");
  await app.page.locator("#page-grid:visible").waitFor();
  return app;
}

const card = (app, index) => app.page.locator(`.page-card[data-page="${index}"]`);

async function selected(app) {
  return app.page.$$eval('.page-card[aria-selected="true"]', (cards) => cards.map((c) => Number(c.dataset.page)));
}

test("Pages : grille des pages, page affichée choisie, double-clic pour l'ouvrir", async () => {
  const app = await openInPagesMode();
  try {
    assert.equal(await app.page.locator(".page-card").count(), 3);
    assert.deepEqual(await selected(app), [0]);
    assert.equal(await app.page.locator("#pages-toolbar").isVisible(), true);
    // Un document garde au moins une page : la seule page choisie peut partir.
    assert.equal(await app.page.isDisabled("#pages-remove"), false);
    await card(app, 2).dblclick();
    await app.page.locator("#page-grid").waitFor({ state: "hidden" });
    assert.equal(await app.page.getAttribute("#mode-read", "aria-current"), "page");
    assert.equal(await app.page.locator("#pages-toolbar").isVisible(), false);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Pages : page blanche après la sélection ; grille, miniatures et pages suivent", async () => {
  const app = await openInPagesMode({ insert_blank_page: [{ ok: pagesView(4, [2]) }] });
  try {
    await card(app, 1).click();
    await app.page.click("#pages-insert");
    await app.page.locator(".page-card").nth(3).waitFor();
    assert.deepEqual(await app.callsTo("insert_blank_page"), [{ id: 1, at: 2 }]);
    assert.deepEqual(await selected(app), [2]);
    assert.equal(await app.page.locator("#thumbs .thumb").count(), 4);
    assert.equal(await app.page.locator("#desk .page").count(), 4);
    assert.equal(await app.page.textContent("#page-count"), "sur 4");
    assert.equal(await app.page.locator("#doc-dirty").isVisible(), true);
    assert.equal(await app.page.isDisabled("#pages-undo"), false);
    // Le document est déjà décrit par la réponse : pas de relecture.
    assert.deepEqual(await app.callsTo("document_view"), []);
  } finally {
    await app.close();
  }
});

test("Pages : Maj+clic choisit une plage, Supprimer la retire ; la dernière page reste", async () => {
  const app = await openInPagesMode({ remove_pages: [{ ok: pagesView(1, [0]) }] });
  try {
    await card(app, 0).click();
    await card(app, 1).click({ modifiers: ["Shift"] });
    assert.deepEqual(await selected(app), [0, 1]);
    assert.equal(await app.page.textContent("#pages-count"), "2 pages choisies");
    await app.page.click("#pages-remove");
    await app.page.locator(".page-card").nth(1).waitFor({ state: "detached" });
    assert.deepEqual(await app.callsTo("remove_pages"), [{ id: 1, pages: [0, 1] }]);
    // Une seule page : elle ne peut plus être supprimée.
    assert.equal(await app.page.isDisabled("#pages-remove"), true);
  } finally {
    await app.close();
  }
});

test("Pages : Ctrl+clic, pivoter dans les deux sens et dupliquer envoient la sélection", async () => {
  const app = await openInPagesMode({
    rotate_pages: [{ ok: pagesView(3, [0, 2], 1) }, { ok: pagesView(3, [0, 2], 2) }],
    duplicate_pages: [{ ok: pagesView(5, [1, 4], 3) }],
  });
  try {
    await card(app, 2).click({ modifiers: ["Control"] });
    assert.deepEqual(await selected(app), [0, 2]);
    await app.page.click("#pages-rotate-right");
    await app.page.locator("#pages-rotate-right:enabled").waitFor();
    await app.page.click("#pages-rotate-left");
    await app.page.locator("#pages-rotate-left:enabled").waitFor();
    assert.deepEqual(await app.callsTo("rotate_pages"), [
      { id: 1, pages: [0, 2], turns: 1 },
      { id: 1, pages: [0, 2], turns: 3 },
    ]);
    await app.page.click("#pages-duplicate");
    await app.page.locator(".page-card").nth(4).waitFor();
    assert.deepEqual(await app.callsTo("duplicate_pages"), [{ id: 1, pages: [0, 2] }]);
    // Les copies deviennent la sélection.
    assert.deepEqual(await selected(app), [1, 4]);
  } finally {
    await app.close();
  }
});

/** Glisse la carte `from` jusqu'au point (fraction de la largeur) de la carte `onto`. */
async function drag(app, from, onto, fraction) {
  const start = await card(app, from).boundingBox();
  const end = await card(app, onto).boundingBox();
  await app.page.mouse.move(start.x + start.width / 2, start.y + start.height / 2);
  await app.page.mouse.down();
  await app.page.mouse.move(end.x + end.width * fraction, end.y + end.height / 2, { steps: 8 });
  assert.equal(await app.page.locator(".drop-marker").isVisible(), true);
  await app.page.mouse.up();
}

test("Pages : glisser une page la place ailleurs ; la déposer sur elle-même ne fait rien", async () => {
  const app = await openInPagesMode({ move_pages: [{ ok: pagesView(3, [2]) }] });
  try {
    // Sur elle-même : aucun appel.
    await drag(app, 1, 1, 0.8);
    assert.deepEqual(await app.callsTo("move_pages"), []);
    // Page 1 déposée sur la moitié droite de la page 3 : à la fin.
    await drag(app, 0, 2, 0.8);
    await app.page.locator('.page-card[data-page="2"][aria-selected="true"]').waitFor();
    assert.deepEqual(await app.callsTo("move_pages"), [{ id: 1, pages: [0], to: 3 }]);
    assert.equal(await app.page.locator(".drop-marker").isVisible(), false);
  } finally {
    await app.close();
  }
});

test("Pages : annuler relit le document et refait la grille", async () => {
  const app = await openInPagesMode({
    insert_blank_page: [{ ok: pagesView(4, [3]) }],
    undo_edit: [{ ok: editView(3, { layout: 2, revision: 2 }) }],
    document_view: [{ ok: documentView(3) }],
  });
  try {
    await app.page.click("#pages-insert");
    await app.page.locator(".page-card").nth(3).waitFor();
    await app.page.keyboard.press("Control+z");
    await app.page.locator(".page-card").nth(3).waitFor({ state: "detached" });
    assert.deepEqual(await app.callsTo("document_view"), [{ id: 1 }]);
    assert.equal(await app.page.locator("#thumbs .thumb").count(), 3);
    // La page choisie (la page blanche, disparue) est ramenée dans le document.
    assert.deepEqual(await selected(app), [2]);
  } finally {
    await app.close();
  }
});

test("Pages : opération refusée par Rust, message affiché, grille inchangée", async () => {
  const message = "la page 1 porte du texte ajouté, souligné ou barré, qui ne pivote pas encore avec elle";
  const app = await openInPagesMode({ rotate_pages: [{ error: { kind: "invalid_request", message } }] });
  try {
    await app.page.click("#pages-rotate-right");
    await app.page.locator("#toast.error").waitFor();
    assert.match(await app.page.textContent("#toast"), /Opération impossible : la page 1 porte du texte/);
    assert.equal(await app.page.locator(".page-card").count(), 3);
  } finally {
    await app.close();
  }
});

test("Pages : clavier — Ctrl+A, flèches, Suppr", async () => {
  const app = await openInPagesMode({ remove_pages: [{ ok: pagesView(2, [1]) }] });
  try {
    await app.page.keyboard.press("Control+a");
    assert.equal(await app.page.textContent("#pages-count"), "3 pages choisies");
    // Tout choisi : impossible de tout supprimer.
    await app.page.keyboard.press("Delete");
    assert.deepEqual(await app.callsTo("remove_pages"), []);
    await app.page.keyboard.press("Home");
    await app.page.keyboard.press("ArrowRight");
    assert.deepEqual(await selected(app), [1]);
    await app.page.keyboard.press("Delete");
    await app.page.locator(".page-card").nth(2).waitFor({ state: "detached" });
    assert.deepEqual(await app.callsTo("remove_pages"), [{ id: 1, pages: [1] }]);
  } finally {
    await app.close();
  }
});
