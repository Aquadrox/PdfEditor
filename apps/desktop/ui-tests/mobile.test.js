// Téléphone (A-01) : disposition compacte, tiroir des pages, feuilles,
// pincer pour zoomer, double tapotement, tapotement pour poser du texte,
// bouton Retour d'Android (historique du WebView).

import assert from "node:assert/strict";
import { test } from "node:test";

import { PHONE, documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0, 0, 0, 0], can_undo: false, can_redo: false, dirty: false };

async function openOnPhone(commands = {}) {
  const app = await launch(
    {
      initial_files: [{ ok: ["/docs/contrat.pdf"] }],
      open_document: [{ ok: documentView(4) }],
      edit_view: [{ ok: EMPTY }],
      ...commands,
    },
    { device: PHONE },
  );
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  return app;
}

/** Zoom affiché dans la pastille, en pour cent. */
async function zoomPercent(page) {
  return Number.parseInt((await page.textContent("#zoom-value")).replace(/\D/g, ""), 10);
}

/** Gestes à plusieurs doigts : événements tactiles envoyés par Chromium (CDP). */
async function touch(page) {
  const cdp = await page.context().newCDPSession(page);
  const send = (type, points) => cdp.send("Input.dispatchTouchEvent", { type, touchPoints: points });
  return {
    /** Deux doigts écartés de `from` à `to` px autour de (x, y). */
    async pinch(x, y, from, to, steps = 6) {
      const points = (d) => [
        { x: x - d / 2, y, id: 1 },
        { x: x + d / 2, y, id: 2 },
      ];
      await send("touchStart", points(from));
      for (let i = 1; i <= steps; i += 1) await send("touchMove", points(from + ((to - from) * i) / steps));
      await send("touchEnd", []);
    },
    /** Un doigt qui glisse verticalement de `dy` px (défilement). */
    async drag(x, y, dy, steps = 8) {
      await send("touchStart", [{ x, y, id: 1 }]);
      for (let i = 1; i <= steps; i += 1) await send("touchMove", [{ x, y: y + (dy * i) / steps, id: 1 }]);
      await send("touchEnd", []);
    },
  };
}

/** Centre de la première page à l'écran. */
async function pageCenter(page) {
  const box = await page.locator(".page").first().boundingBox();
  return { x: box.x + box.width / 2, y: box.y + Math.min(box.height / 2, 300) };
}

test("téléphone : modes en bas, barre du haut réduite, pastille de page", async () => {
  const app = await openOnPhone();
  try {
    const viewport = app.page.viewportSize();
    const modes = await app.page.locator(".modes").boundingBox();
    assert.ok(modes.y + modes.height >= viewport.height - 1, "barre des modes en bas de l'écran");
    for (const id of ["#mode-read", "#mode-edit", "#mode-sign", "#mode-ai"]) {
      const box = await app.page.locator(id).boundingBox();
      assert.ok(box.height >= 44 && box.width >= 44, `${id} : cible de 44 px au moins`);
    }
    // Pages et plan rangés dans le tiroir, fermé ; son bouton est là.
    assert.equal(await app.page.locator("#sidebar").isVisible(), false);
    assert.equal(await app.page.locator("#sidebar-toggle").isVisible(), true);
    assert.equal(await app.page.locator("#statusbar").isVisible(), true);
    // Aucune barre de défilement horizontale pour la fenêtre.
    assert.equal(await app.page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth), true);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("téléphone : le tiroir des pages s'ouvre, mène à une page et se ferme", async () => {
  const app = await openOnPhone();
  try {
    await app.page.tap("#sidebar-toggle");
    await app.page.locator("#sidebar.open").waitFor();
    assert.equal(await app.page.getAttribute("#sidebar-toggle", "aria-expanded"), "true");
    await app.page.tap("#tab-pages");
    await app.page.locator(".thumb").nth(2).tap();
    // Le tiroir s'efface pour laisser voir la page choisie.
    await app.page.waitForFunction(() => !document.querySelector("#sidebar").classList.contains("open"));
    assert.equal(await app.page.locator("#scrim").isVisible(), false);
    await app.page.waitForFunction(() => document.querySelector("#page-input").value === "3");
    // Le voile ferme aussi le tiroir.
    await app.page.tap("#sidebar-toggle");
    await app.page.locator("#sidebar.open").waitFor();
    await app.page.mouse.click(400, 400);
    await app.page.waitForFunction(() => !document.querySelector("#sidebar").classList.contains("open"));
  } finally {
    await app.close();
  }
});

test("téléphone : le bouton Retour ferme le tiroir, puis les Réglages", async () => {
  const app = await openOnPhone();
  try {
    await app.page.tap("#sidebar-toggle");
    await app.page.locator("#sidebar.open").waitFor();
    await app.page.goBack();
    await app.page.waitForFunction(() => !document.querySelector("#sidebar").classList.contains("open"));
    assert.equal(await app.page.locator("#doc-name").textContent(), "contrat.pdf");

    await app.page.tap("#menu-button");
    await app.page.tap("#menu-settings");
    await app.page.locator("#settings-view:visible").waitFor();
    await app.page.goBack();
    await app.page.locator("#settings-view").waitFor({ state: "hidden" });
    assert.equal(await app.page.locator("#doc-name").isVisible(), true);
    // Fermé autrement (voile), le tiroir ne laisse pas d'étape : rien de plus à remonter.
    await app.page.tap("#sidebar-toggle");
    await app.page.locator("#sidebar.open").waitFor();
    await app.page.mouse.click(400, 400);
    await app.page.waitForFunction(() => !document.querySelector("#sidebar").classList.contains("open"));
    assert.equal(await app.page.evaluate(() => history.state?.pdeBack ?? null), null);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("téléphone : pincer pour zoomer, double tapotement pour grossir puis revenir", async () => {
  const app = await openOnPhone();
  try {
    const fitted = await zoomPercent(app.page);
    const { x, y } = await pageCenter(app.page);
    const fingers = await touch(app.page);
    await fingers.pinch(x, y, 100, 200);
    await app.page.waitForFunction((before) => {
      const now = Number.parseInt(document.querySelector("#zoom-value").textContent.replace(/\D/g, ""), 10);
      return now >= before * 1.9;
    }, fitted);
    // Plus d'agrandissement provisoire : le vrai zoom est appliqué.
    assert.equal(await app.page.evaluate(() => document.querySelector(".pages").style.transform), "");

    // Double tapotement hors de la largeur : retour à la largeur de l'écran.
    await app.page.touchscreen.tap(x, y);
    await app.page.touchscreen.tap(x, y);
    await app.page.waitForFunction(
      (before) => Number.parseInt(document.querySelector("#zoom-value").textContent.replace(/\D/g, ""), 10) === before,
      fitted,
    );
    // À la largeur : la page grossit sous le doigt.
    await app.page.waitForTimeout(350);
    await app.page.touchscreen.tap(x, y);
    await app.page.touchscreen.tap(x, y);
    await app.page.waitForFunction((before) => {
      const now = Number.parseInt(document.querySelector("#zoom-value").textContent.replace(/\D/g, ""), 10);
      return now >= before * 1.9;
    }, fitted);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("téléphone, Éditer : faire défiler ne pose pas de texte, un tapotement si", async () => {
  const app = await openOnPhone();
  try {
    await app.page.tap("#mode-edit");
    await app.page.locator("#edit-toolbar:visible").waitFor();
    // Outils sur une bande, réglages et Annuler sur une autre ; pas de
    // Propriétés sans sélection (la page reste dégagée).
    const tools = await app.page.locator("#tool-text").boundingBox();
    const undo = await app.page.locator("#edit-undo").boundingBox();
    assert.ok(undo.y >= tools.y + tools.height - 1, "Annuler sous les outils");
    assert.equal(await app.page.locator("#properties").isVisible(), false);

    await app.page.locator(".edit-layer.active").first().waitFor();
    const { x, y } = await pageCenter(app.page);
    const fingers = await touch(app.page);
    await fingers.drag(x, y + 100, -200);
    await app.page.waitForTimeout(200);
    assert.equal(await app.page.locator(".text-editor").count(), 0, "un défilement n'ouvre pas de saisie");

    // La page a défilé : tapotement au milieu de l'écran, sur une page.
    const viewport = app.page.viewportSize();
    await app.page.touchscreen.tap(viewport.width / 2, viewport.height / 2);
    await app.page.locator(".text-editor").waitFor();
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("téléphone : Signatures en feuille repliée, dépliée d'un appui sur son titre", async () => {
  const app = await openOnPhone({ document_signatures: [{ ok: [] }] });
  try {
    await app.page.tap("#mode-sign");
    await app.page.locator("#signatures-panel:visible").waitFor();
    const toggle = app.page.locator("#signatures-panel .sheet-toggle");
    assert.equal(await toggle.getAttribute("aria-expanded"), "false");
    assert.equal(await app.page.locator("#signatures-empty").isVisible(), false);
    await app.page.tap("#signatures-title");
    assert.equal(await toggle.getAttribute("aria-expanded"), "true");
    assert.equal(await app.page.locator("#signatures-empty").isVisible(), true);
    // La feuille repose au-dessus de la barre des modes.
    const sheet = await app.page.locator("#signatures-panel").boundingBox();
    const modes = await app.page.locator(".modes").boundingBox();
    assert.ok(Math.abs(sheet.y + sheet.height - modes.y) <= 1);
  } finally {
    await app.close();
  }
});

test("PC : ni tiroir ni feuilles, la barre d'outils reste sur une ligne", async () => {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY }],
  });
  try {
    await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
    assert.equal(await app.page.locator("#sidebar").isVisible(), true);
    assert.equal(await app.page.locator("#sidebar-toggle").isVisible(), false);
    await app.page.click("#mode-edit");
    const text = await app.page.locator("#tool-text").boundingBox();
    const font = await app.page.locator("#edit-toolbar .font-select").boundingBox();
    assert.ok(Math.abs(text.y + text.height / 2 - (font.y + font.height / 2)) < 4, "outils et réglages alignés");
    assert.equal(await app.page.locator("#properties .sheet-toggle").isVisible(), false);
  } finally {
    await app.close();
  }
});
