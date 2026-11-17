// Mode Pages (P-01, P-02) : grille des pages et barre d'outils — page
// blanche, dupliquer, pivoter, supprimer, réordonner en glissant. Chaque
// opération est une entrée de l'historique d'édition (annuler, rétablir) ;
// le document est ensuite décrit à nouveau par la session.

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
    for (const id of ["pages-duplicate", "pages-rotate-left", "pages-rotate-right"]) byId(id).disabled = none;
    // Un document garde au moins une page.
    byId("pages-remove").disabled = none || pages.length >= this.count;
    byId("pages-insert").disabled = this.busy || !this.session.doc;
    byId("pages-undo").disabled = !this.session.view.can_undo;
    byId("pages-redo").disabled = !this.session.view.can_redo;
    byId("pages-count").textContent = pages.length > 1 ? t("pages.selected", { n: pages.length }) : "";
  }

  remove() {
    if (byId("pages-remove").disabled) return;
    this.run("remove_pages", { pages: this.selection.pages });
  }

  /** Opération sur les pages ; ensuite, les pages qu'elle désigne sont choisies. */
  async run(command, args) {
    if (this.busy || !this.session.doc) return;
    this.busy = true;
    this.refresh();
    try {
      const result = await invoke(command, { id: this.session.doc.id, ...args });
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
