// Mode Éditer : assemble la session d'édition, la couche posée sur les
// pages, la barre d'outils et le panneau Propriétés ; touches propres au
// mode (Suppr, flèches, Entrée, Échap).

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { EditLayer } from "./edit-layer.js";
import { EditSession } from "./edit-session.js";
import { duplicated, nudged, withStyle } from "./edit-state.js";
import { attachProperties, renderSwatches } from "./properties.js";
import { attachStyleControls } from "./style-controls.js";

const fail = (error) => toast(t("edit.failed", { message: error?.message ?? String(error) }), "error");

export class EditMode {
  /**
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {(view: object, changedPages: number[]) => void} onChange
   */
  constructor(viewer, onChange) {
    this.viewer = viewer;
    this.session = new EditSession();
    this.layer = new EditLayer(this.session, viewer);
    this.active = false;
    const pages = () => viewer.doc?.pages ?? [];
    renderSwatches();
    this.styles = attachStyleControls(
      document,
      () => this.session.element(this.session.selected)?.element.style ?? this.session.style,
      (changes) => this.applyStyle(changes),
    );
    this.properties = attachProperties(this.session, pages, {
      change: (id, element) => this.session.change(id, element).catch(fail),
      remove: (id) => this.session.remove(id).catch(fail),
      duplicate: (item) => this.session.add(duplicated(item.element, pages()[item.element.page])).catch(fail),
      close: () => this.session.select(null),
    });
    byId("tool-text").addEventListener("click", () => {
      this.layer.tool = "text";
    });
    this.session.subscribe((view, changed) => {
      if (changed.length > 0) viewer.setRevisions(view.revisions, changed);
      this.layer.refresh();
      this.styles.refresh();
      this.properties.refresh(this.active);
      byId("edit-undo").disabled = !view.can_undo;
      byId("edit-redo").disabled = !view.can_redo;
      onChange(view, changed);
    });
  }

  async load(doc) {
    await this.session.load(doc);
  }

  setActive(active) {
    this.active = active;
    byId("edit-toolbar").hidden = !active;
    this.viewer.desk.classList.toggle("editing", active);
    if (!active) this.session.select(null);
    this.layer.setActive(active);
    this.properties.refresh(active);
  }

  /** Style choisi : zone sélectionnée, et prochaines zones. */
  applyStyle(changes) {
    this.session.style = { ...this.session.style, ...changes };
    const item = this.session.element(this.session.selected);
    if (item) this.session.change(item.id, withStyle(item.element, changes)).catch(fail);
    else this.styles.refresh();
  }

  undo() {
    this.layer.commitText();
    if (this.session.view.can_undo) this.session.undo().catch(fail);
  }

  redo() {
    this.layer.commitText();
    if (this.session.view.can_redo) this.session.redo().catch(fail);
  }

  /** Touche hors champ de saisie ; `true` si le mode l'a utilisée. */
  handleKey(event) {
    if (!this.active) return false;
    const item = this.session.element(this.session.selected);
    if (!item) return false;
    if (event.key === "Delete" || event.key === "Backspace") {
      this.session.remove(item.id).catch(fail);
    } else if (event.key === "Escape") {
      this.session.select(null);
    } else if (event.key === "Enter") {
      this.layer.editSelected();
    } else {
      const page = this.viewer.doc.pages[item.element.page];
      const element = nudged(item.element, event.key, event.shiftKey, page);
      if (!element) return false;
      this.session.change(item.id, element).catch(fail);
    }
    return true;
  }
}
