// Annotations dessinées (E-04) : formes, dessin à main levée, marquage du
// texte sélectionné, options d'outil, poignées, panneau.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const RED = { r: 0xc6, g: 0x28, b: 0x28 };
const EMPTY = { elements: [], revisions: [0, 0, 0], can_undo: false, can_redo: false, dirty: false };
const RUN = { text: "Bonjour PdfEditor", x: 72, y: 60, width: 200, height: 14 };

function view(element, bounds) {
  return { elements: [{ id: 5, element, bounds }], revisions: [1, 0, 0], can_undo: true, can_redo: false, dirty: true };
}

const RECT = { kind: "shape", page: 0, shape: "rectangle", start: { x: 100, y: 200 }, end: { x: 300, y: 300 }, color: RED, width: 2, filled: false };
const RECT_VIEW = view(RECT, { x: 99, y: 199, width: 202, height: 102 });
const ARROW = { ...RECT, shape: "arrow", start: { x: 100, y: 200 }, end: { x: 300, y: 200 } };
const ARROW_VIEW = view(ARROW, { x: 99, y: 192, width: 202, height: 16 });

async function openInEditMode(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
    page_text: [{ ok: { runs: [RUN] } }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.click("#mode-edit");
  await app.page.locator("#edit-toolbar:visible").waitFor();
  return app;
}

async function waitForCall(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

/** Point de la page 1 (points) → pixels écran. */
async function screenPoint(app, x, y) {
  const box = await app.page.locator(".page .edit-layer").first().boundingBox();
  return { x: box.x + (x / 595) * box.width, y: box.y + (y / 842) * box.height };
}

async function drag(app, from, to, { steps = 5, shift = false } = {}) {
  const a = await screenPoint(app, from.x, from.y);
  const b = await screenPoint(app, to.x, to.y);
  await app.page.mouse.move(a.x, a.y);
  if (shift) await app.page.keyboard.down("Shift");
  await app.page.mouse.down();
  for (let i = 1; i <= steps; i += 1) {
    await app.page.mouse.move(a.x + ((b.x - a.x) * i) / steps, a.y + ((b.y - a.y) * i) / steps);
  }
  await app.page.mouse.up();
  if (shift) await app.page.keyboard.up("Shift");
}

const near = (a, b, tolerance = 1.5) => Math.abs(a - b) <= tolerance;

test("outil Forme : un glisser trace un rectangle, ses options s'affichent", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 5, view: RECT_VIEW } }] });
  try {
    await app.page.click("#tool-shape");
    assert.equal(await app.page.locator('[data-tool-options="shape"]').isVisible(), true);
    assert.equal(await app.page.locator('[data-tool-options="text"]').isVisible(), false);
    assert.equal(await app.page.locator("#tool-shape").getAttribute("aria-pressed"), "true");
    await drag(app, { x: 100, y: 200 }, { x: 300, y: 300 });
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.kind, e.shape, e.page, e.width, e.filled], ["shape", "rectangle", 0, 2, false]);
    assert.deepEqual(e.color, RED);
    assert.ok(near(e.start.x, 100) && near(e.start.y, 200) && near(e.end.x, 300) && near(e.end.y, 300), JSON.stringify(e));
    // Aperçu retiré une fois le tracé fini.
    assert.equal(await app.page.locator(".draft").count(), 0);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Forme : flèche, épaisseur et remplissage choisis avant de tracer ; Maj contraint l'angle", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 5, view: ARROW_VIEW } }] });
  try {
    await app.page.click("#tool-shape");
    await app.page.click('#edit-toolbar [data-annot="shape"][data-value="arrow"]');
    await app.page.fill('#edit-toolbar [data-tool-options="shape"] [data-annot="width"]', "4");
    await app.page.press('#edit-toolbar [data-tool-options="shape"] [data-annot="width"]', "Tab");
    await app.page.click('#edit-toolbar [data-tool-options="shape"] [data-annot="color"][data-value="#2747b8"]');
    await drag(app, { x: 100, y: 200 }, { x: 300, y: 212 }, { shift: true });
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.shape, e.width], ["arrow", 4]);
    assert.deepEqual(e.color, { r: 0x27, g: 0x47, b: 0xb8 });
    // Maj : trait horizontal.
    assert.ok(near(e.end.y, e.start.y, 0.01), JSON.stringify(e));
  } finally {
    await app.close();
  }
});

test("Dessin : le trait suit la souris ; Échap annule un tracé", async () => {
  const app = await openInEditMode({ add_element: [{ ok: { id: 5, view: RECT_VIEW } }] });
  try {
    await app.page.click("#tool-ink");
    await drag(app, { x: 100, y: 100 }, { x: 200, y: 150 }, { steps: 10 });
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.equal(e.kind, "ink");
    assert.ok(e.points.length >= 5, `${e.points.length} points`);
    assert.ok(near(e.points[0].x, 100) && near(e.points.at(-1).x, 200), JSON.stringify(e.points));

    // Un simple clic (pour désélectionner) ne crée rien.
    const tap = await screenPoint(app, 400, 600);
    await app.page.mouse.click(tap.x, tap.y);
    assert.equal((await app.callsTo("add_element")).length, 1);

    // Échap pendant le tracé : rien n'est ajouté.
    const a = await screenPoint(app, 300, 300);
    await app.page.mouse.move(a.x, a.y);
    await app.page.mouse.down();
    await app.page.mouse.move(a.x + 40, a.y + 20);
    await app.page.keyboard.press("Escape");
    await app.page.mouse.up();
    assert.equal((await app.callsTo("add_element")).length, 1);
    assert.equal(await app.page.locator(".draft").count(), 0);
  } finally {
    await app.close();
  }
});

const RUN2 = { text: "Seconde ligne du contrat", x: 72, y: 80, width: 180, height: 14 };
/** Fin de ligne en segment séparé, à boîte plus basse (cas réel : « nouvel » + « le »). */
const RUN_TAIL = { text: "le", x: 274, y: 63, width: 12, height: 10 };

async function openWithTwoLines(commands = {}) {
  return openInEditMode({ page_text: [{ ok: { runs: [RUN, RUN2] } }], ...commands });
}

/** Glisser sur le texte de la page 1 (points de la page). */
async function dragText(app, from, to) {
  await app.page.locator(".text-layer span").nth(1).waitFor({ state: "attached" });
  await drag(app, from, to, { steps: 6 });
}

test("Surligner : relâcher après la fin de la ligne ne déborde pas sur les suivantes", async () => {
  const app = await openWithTwoLines({ add_element: [{ ok: { id: 5, view: RECT_VIEW } }] });
  try {
    await app.page.click("#tool-markup");
    await app.page.click('#edit-toolbar [data-annot="mark"][data-value="underline"]');
    // Du début de la ligne 1 jusqu'à l'espace vide à sa droite.
    await dragText(app, { x: RUN.x + 1, y: RUN.y + 7 }, { x: 450, y: RUN.y + 7 });
    const [added] = await waitForCall(app, "add_element");
    const e = added.element;
    assert.deepEqual([e.kind, e.mark, e.page, e.areas.length], ["markup", "underline", 0, 1]);
    const area = e.areas[0];
    assert.ok(near(area.top_left.x, RUN.x, 2) && near(area.width, RUN.width, 3), JSON.stringify(area));
    // Hauteur exacte du texte dans le PDF (pas celle de la police d'affichage).
    assert.ok(near(area.top_left.y, RUN.y, 0.01) && near(area.height, RUN.height, 0.01), JSON.stringify(area));
    assert.equal(await app.page.evaluate(() => getSelection().isCollapsed), true);
  } finally {
    await app.close();
  }
});

test("Surligner jusqu'au bout de la ligne prend aussi son dernier segment, même plus bas", async () => {
  const app = await openInEditMode({
    page_text: [{ ok: { runs: [RUN, RUN_TAIL, RUN2] } }],
    add_element: [{ ok: { id: 5, view: RECT_VIEW } }],
  });
  try {
    await app.page.click("#tool-markup");
    await dragText(app, { x: RUN.x + 1, y: RUN.y + 3 }, { x: 450, y: RUN.y + 3 });
    const [added] = await waitForCall(app, "add_element");
    const [area] = added.element.areas;
    assert.equal(added.element.areas.length, 1);
    assert.ok(near(area.top_left.x + area.width, RUN_TAIL.x + RUN_TAIL.width, 2), JSON.stringify(area));
  } finally {
    await app.close();
  }
});

/** Segments réels d'un contrat (interligne serré : 18 pt pour un corps de 14,65). */
const CONTRACT = [
  { text: "Article 1 : Objet", x: 72, y: 106.59, width: 84.86, height: 14.65 },
  { text: "Le present contrat a pour objet la conception de la nouvelle", x: 72, y: 124.59, width: 308.32, height: 14.65 },
  { text: "identite graphique du Client, telle que decrite dans l’annexe A.", x: 72, y: 142.59, width: 326.03, height: 14.65 },
];

test("Surligner, interligne serré : relâcher à droite de la ligne garde la ligne et sa fin exacte", async () => {
  const app = await openInEditMode({
    page_text: [{ ok: { runs: CONTRACT } }],
    add_element: [{ ok: { id: 5, view: RECT_VIEW } }],
  });
  try {
    await app.page.click("#tool-markup");
    const line = CONTRACT[1];
    // Bas de la ligne, bien à droite de son texte (ligne suivante plus longue).
    await dragText(app, { x: 180, y: line.y + 10 }, { x: 450, y: line.y + 11 });
    const [added] = await waitForCall(app, "add_element");
    assert.equal(added.element.areas.length, 1, JSON.stringify(added.element.areas));
    const [area] = added.element.areas;
    assert.ok(near(area.top_left.y, line.y, 0.01), JSON.stringify(area));
    assert.ok(near(area.top_left.x + area.width, line.x + line.width, 0.01), JSON.stringify(area));
  } finally {
    await app.close();
  }
});

test("Surligner sur deux lignes : une bande par ligne, du point de départ au point d'arrivée", async () => {
  const app = await openWithTwoLines({ add_element: [{ ok: { id: 5, view: RECT_VIEW } }] });
  try {
    await app.page.click("#tool-markup");
    await dragText(app, { x: RUN.x + RUN.width / 2, y: RUN.y + 7 }, { x: RUN2.x + RUN2.width / 2, y: RUN2.y + 7 });
    const [added] = await waitForCall(app, "add_element");
    const [first, second] = added.element.areas;
    assert.equal(added.element.areas.length, 2);
    assert.equal(added.element.mark, "highlight");
    // Ligne 1 : de son milieu à sa fin ; ligne 2 : de son début à son milieu.
    assert.ok(near(first.top_left.x, RUN.x + RUN.width / 2, 15) && near(first.top_left.x + first.width, RUN.x + RUN.width, 3), JSON.stringify(first));
    assert.ok(near(second.top_left.x, RUN2.x, 2) && near(second.width, RUN2.width / 2, 15), JSON.stringify(second));
    assert.ok(near(second.top_left.y, RUN2.y, 0.01));
  } finally {
    await app.close();
  }
});

test("Surligner : un clic sans glisser n'ajoute rien", async () => {
  const app = await openWithTwoLines();
  try {
    await app.page.click("#tool-markup");
    await dragText(app, { x: RUN.x + 20, y: RUN.y + 7 }, { x: RUN.x + 20, y: RUN.y + 7 });
    await app.page.waitForTimeout(200);
    assert.deepEqual(await app.callsTo("add_element"), []);
  } finally {
    await app.close();
  }
});

test("forme sélectionnée : panneau Forme, épaisseur, remplissage et forme modifiables", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: RECT_VIEW }], update_element: [{ ok: RECT_VIEW }] });
  try {
    await app.page.locator(".edit-layer .element").first().click();
    await app.page.locator("#prop-annot-fields:visible").waitFor();
    assert.equal(await app.page.textContent("#properties-title"), "Forme");
    assert.equal(await app.page.locator(".element.selected .handle").count(), 4);
    assert.equal(await app.page.locator('#prop-annot-fields [data-annot="filled"]').isVisible(), true);
    assert.equal(await app.page.locator('#prop-annot-fields [data-annot="mark"]').first().isVisible(), false);
    await app.page.fill('#prop-annot-fields [data-annot="width"]', "6");
    await app.page.press('#prop-annot-fields [data-annot="width"]', "Tab");
    await app.page.click('#prop-annot-fields [data-annot="filled"]');
    await app.page.click('#prop-annot-fields [data-annot="shape"][data-value="ellipse"]');
    const [width, filled, ellipse] = await waitForCall(app, "update_element", 3);
    assert.equal(width.element.width, 6);
    assert.equal(filled.element.filled, true);
    assert.equal(ellipse.element.shape, "ellipse");
    // Le style de texte ne la touche pas.
    await app.page.click('#edit-toolbar [data-style="bold"]');
    assert.equal((await app.callsTo("update_element")).length, 3);
  } finally {
    await app.close();
  }
});

test("flèche : l'extrémité se tire par sa poignée ; déplacer la forme déplace ses deux points", async () => {
  const app = await openInEditMode({ edit_view: [{ ok: ARROW_VIEW }], update_element: [{ ok: ARROW_VIEW }] });
  try {
    await app.page.locator(".edit-layer .element").first().click();
    assert.equal(await app.page.locator(".element.selected .handle").count(), 2);
    const handle = await app.page.locator(".element.selected .handle.end").boundingBox();
    const target = await screenPoint(app, 300, 260);
    await app.page.mouse.move(handle.x + handle.width / 2, handle.y + handle.height / 2);
    await app.page.mouse.down();
    await app.page.mouse.move(target.x, target.y, { steps: 4 });
    await app.page.mouse.up();
    const [end] = await waitForCall(app, "update_element");
    assert.deepEqual(end.element.start, ARROW.start);
    assert.ok(near(end.element.end.x, 300, 2) && near(end.element.end.y, 260, 2), JSON.stringify(end.element.end));

    // Glisser le contour : les deux points bougent ensemble.
    const from = await screenPoint(app, 200, 200);
    const to = await screenPoint(app, 250, 230);
    await app.page.mouse.move(from.x, from.y);
    await app.page.mouse.down();
    await app.page.mouse.move(to.x, to.y, { steps: 4 });
    await app.page.mouse.up();
    const [, moved] = await waitForCall(app, "update_element", 2);
    const { start, end: e2 } = moved.element;
    assert.ok(near(start.x, 150, 2) && near(start.y, 230, 2) && near(e2.x, 350, 2) && near(e2.y, 230, 2), JSON.stringify(moved.element));
  } finally {
    await app.close();
  }
});
