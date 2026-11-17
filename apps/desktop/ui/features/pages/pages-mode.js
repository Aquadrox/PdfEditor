// Mode Pages (P-01 à P-03) : grille des pages et barre d'outils — page
// blanche, pages d'autres PDF (bouton ou fichiers déposés), dupliquer,
// pivoter, supprimer, extraire, réordonner en glissant. Chaque opération
// est une entrée de l'historique d'édition (annuler, rétablir) ; le
// document est ensuite décrit à nouveau par la session. Extraire écrit un
// nouveau fichier sans changer le document.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { PageGrid } from "./page-grid.js";
import { clamped, insertionPoint, moved, selectOnly, selectPages } from "./page-selection.js";

const fail = (error) => toast(t("pages.failed", { message: error?.message ?? String(error) }), "error");

export class PagesMode {
  /**
   * @param {import("../edit/edit-session.js").EditSession} session
   * @param {(page: number) => void} onOpen page à ouvrir en mode Lire
   */
  constructor(session, onOpen) {
    this.session = session;
    this.active = false;
    this.busy = false;
    this.grid = new PageGrid(byId("page-grid"), {
      onSelection: () => this.refresh(),
      onMove: (pages, to) => this.run("move_pages", { pages, to }),
      onOpen,
    });
    const actions = {
      "pages-insert": () => this.run("insert_blank_page", { at: insertionPoint(this.selection, this.count) }),
      // Dialogue de Rust ; null si annulé.
      "pages-insert-pdf": () => this.run("insert_pdf", { at: insertionPoint(this.selection, this.count) }),
      "pages-extract": () => this.extract(),
      "pages-duplicate": () => this.run("duplicate_pages", { pages: this.selection.pages }),
      "pages-rotate-left": () => this.run("rotate_pages", { pages: this.selection.pages, turns: 3 }),
      "pages-rotate-right": () => this.run("rotate_pages", { pages: this.selection.pages, turns: 1 }),
      "pages-remove": () => this.remove(),
      "pages-undo": () => session.undo().catch(fail),
      "pages-redo": () => session.redo().catch(fail),
    };
    for (const [id, action] of Object.entries(actions)) byId(id).addEventListener("click", action);
    session.subscribe((view, changed) => {
      if (changed.length > 0) this.grid.setRevisions(view.revisions, changed);
      this.refresh();
    });
  }

  get selection() {
    return this.grid.selection;
  }

  get count() {
    return this.grid.cards.length;
  }

  /** Document (re)décrit : grille refaite, sélection gardée dans ses pages. */
  load(doc) {
    if (!doc) return this.grid.clear();
    this.grid.load(doc, this.session.view.revisions);
    this.grid.setSelection(clamped(this.selection, doc.pages.length));
    this.refresh();
  }

  /** Mode affiché ou quitté ; `page` : page choisie en entrant. */
  setActive(active, page = 0) {
    this.active = active;
    byId("pages-toolbar").hidden = !active;
    byId("page-grid").hidden = !active;
    if (!active) return;
    this.grid.setSelection(selectOnly(Math.min(page, Math.max(0, this.count - 1))));
    this.refresh();
    byId("page-grid").focus({ preventScroll: true });
  }

  /** Première page choisie (page à afficher en quittant le mode). */
  get firstSelected() {
    return this.selection.pages[0] ?? 0;
  }

  refresh() {
    const { pages } = this.selection;
    const none = pages.length === 0 || this.busy;
    for (const id of ["pages-duplicate", "pages-rotate-left", "pages-rotate-right", "pages-extract"]) {
      byId(id).disabled = none;
    }
    // Un document garde au moins une page.
    byId("pages-remove").disabled = none || pages.length >= this.count;
    byId("pages-insert").disabled = this.busy || !this.session.doc;
    byId("pages-insert-pdf").disabled = this.busy || !this.session.doc;
    byId("pages-undo").disabled = !this.session.view.can_undo;
    byId("pages-redo").disabled = !this.session.view.can_redo;
    byId("pages-count").textContent = pages.length > 1 ? t("pages.selected", { n: pages.length }) : "";
  }

  remove() {
    if (byId("pages-remove").disabled) return;
    this.run("remove_pages", { pages: this.selection.pages });
  }

  /** PDF déposés sur la grille : leurs pages sont insérées là où ils tombent. */
  insertFiles(paths, point = null) {
    const at = point ? this.grid.indexAt(point) : insertionPoint(this.selection, this.count);
    return this.run("insert_pdf_files", { paths, at });
  }

  /** Pages choisies écrites dans un nouveau PDF (dialogue de Rust). */
  async extract() {
    if (this.busy || this.selection.pages.length === 0) return;
    this.busy = true;
    this.refresh();
    try {
      const saved = await invoke("extract_pages", { id: this.session.doc.id, pages: this.selection.pages });
      if (saved) toast(t("pages.extracted", { name: saved.file_name }));
    } catch (error) {
      fail(error);
    } finally {
      this.busy = false;
      this.refresh();
    }
  }

  /** Opération sur les pages ; ensuite, les pages qu'elle désigne sont choisies. */
  async run(command, args) {
    if (this.busy || !this.session.doc) return;
    this.busy = true;
    this.refresh();
    try {
      const result = await invoke(command, { id: this.session.doc.id, ...args });
      // Dialogue annulé : rien n'a changé.
      if (!result) return;
      await this.session.apply(result.edit, result.document);
      this.grid.setSelection(selectPages(result.selected));
    } catch (error) {
      fail(error);
    } finally {
      this.busy = false;
      this.refresh();
    }
  }

  /** Touches du mode : Suppr, Ctrl+A, flèches, Début, Fin, Entrée. Vrai si traitée. */
  handleKey(event) {
    if (!this.active) return false;
    const ctrl = event.ctrlKey || event.metaKey;
    if (event.key === "Delete" || event.key === "Backspace") {
      this.remove();
      return true;
    }
    if (ctrl && event.key.toLowerCase() === "a") {
      this.grid.selectAll();
      return true;
    }
    if (event.key === "Enter" && this.selection.focus !== null) {
      this.grid.callbacks.onOpen(this.selection.focus);
      return true;
    }
    if (ctrl || event.altKey) return false;
    const next = moved(this.selection, event.key, this.count, this.grid.columns, event.shiftKey);
    if (next === this.selection) return false;
    this.grid.select(next);
    return true;
  }
}
