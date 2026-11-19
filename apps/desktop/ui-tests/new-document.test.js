// Nouveau document vierge et import d'un document Word (E-13) : document
// sans titre, « Enregistrer » qui demande où l'écrire.

import assert from "node:assert/strict";
import { test } from "node:test";

import { documentView, launch } from "./harness.js";

const EMPTY = { elements: [], revisions: [0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const untitled = (name, pages = 1) => ({ ...documentView(pages), file_name: name, title: name, outline: [], untitled: true });

async function waitForCalls(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

test("Nouveau document : format choisi, ouvert sans titre en mode Formulaire, enregistré sous", async () => {
  const app = await launch({
    new_document: [{ ok: untitled("Sans titre.pdf", 2) }],
    edit_view: [{ ok: EMPTY }],
    form_fields: [{ ok: [] }],
    save_document_as: [{ ok: null }],
  });
  try {
    await app.page.click("#home-new");
    await app.page.locator("#new-dialog[open]").waitFor();
    await app.page.selectOption("#new-paper", "letter");
    await app.page.selectOption("#new-orientation", "landscape");
    await app.page.fill("#new-pages", "2");
    await app.page.click('#new-form button[type="submit"]');
    assert.deepEqual(await waitForCalls(app, "new_document"), [{ request: { paper: "letter", landscape: true, pages: 2 } }]);
    await app.page.locator("#doc-name", { hasText: "Sans titre.pdf" }).waitFor();
    await app.page.locator("#form-toolbar:visible").waitFor();
    assert.equal(await app.page.getAttribute("#mode-form", "aria-current"), "page");
    assert.equal(await app.page.locator(".page").count(), 2);
    // Sans titre : « Enregistrer » est actif et demande où écrire.
    assert.equal(await app.page.isDisabled("#save-button"), false);
    await app.page.click("#save-button");
    assert.deepEqual(await waitForCalls(app, "save_document_as"), [{ id: 1 }]);
    assert.equal((await app.callsTo("save_document")).length, 0);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Importer un document Word : converti, ouvert sans titre, remarque affichée", async () => {
  const app = await launch({
    pick_document_to_import: [{ ok: "/docs/bail.docx" }],
    import_document: [{ ok: { document: untitled("bail.pdf"), converter: null, notice: "Mise en page simplifiée : installez LibreOffice." } }],
    edit_view: [{ ok: EMPTY }],
    form_fields: [{ ok: [] }],
  });
  try {
    await app.page.click("#home-import");
    assert.deepEqual(await waitForCalls(app, "import_document"), [{ path: "/docs/bail.docx" }]);
    await app.page.locator("#doc-name", { hasText: "bail.pdf" }).waitFor();
    await app.page.locator("#toast", { hasText: "Mise en page simplifiée" }).waitFor();
    assert.equal((await app.callsTo("open_document")).length, 0);
  } finally {
    await app.close();
  }
});

test("Document Word déposé ou refusé : message clair", async () => {
  const app = await launch({
    import_document: [{ error: { kind: "invalid_request", message: "pour importer un fichier .doc, installez LibreOffice (gratuit) ou Microsoft Word" } }],
  });
  try {
    await app.page.locator("#home-open").waitFor();
    await app.emit("tauri://drag-drop", { paths: ["/docs/ancien.doc"], position: null });
    assert.deepEqual(await waitForCalls(app, "import_document"), [{ path: "/docs/ancien.doc" }]);
    await app.page.locator("#toast.error", { hasText: "installez LibreOffice" }).waitFor();
  } finally {
    await app.close();
  }
});

/** Rapport de contraste WCAG entre deux couleurs CSS « rgb(r, g, b) ». */
function contrast(a, b) {
  const lum = (css) => {
    const [r, g, bl] = css.match(/\d+(\.\d+)?/g).slice(0, 3).map(Number).map((v) => {
      const c = v / 255;
      return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
    });
    return 0.2126 * r + 0.7152 * g + 0.0722 * bl;
  };
  const [hi, lo] = [lum(a), lum(b)].sort((x, y) => y - x);
  return (hi + 0.05) / (lo + 0.05);
}

test("Accueil en thème sombre : « Nouveau document » et « Importer » restent lisibles", async () => {
  const app = await launch();
  try {
    await app.page.locator("#home-new").waitFor();
    for (const theme of ["dark", "light"]) {
      await app.page.evaluate((t) => (document.documentElement.dataset.theme = t), theme);
      // Les boutons changent de couleur en transition : on mesure après.
      await app.page.waitForTimeout(400);
      for (const id of ["#home-new", "#home-import"]) {
        const style = await app.page.$eval(id, (button) => {
          const s = getComputedStyle(button);
          const icon = button.querySelector(".icon").getBoundingClientRect();
          return { color: s.color, background: s.backgroundColor, icon: icon.width };
        });
        const ratio = contrast(style.color, style.background);
        assert.ok(ratio >= 4.5, `${theme} ${id} : contraste ${ratio.toFixed(2)} (${style.color} sur ${style.background})`);
        assert.ok(style.icon <= 20, `${theme} ${id} : icône de ${style.icon} px`);
      }
    }
  } finally {
    await app.close();
  }
});
