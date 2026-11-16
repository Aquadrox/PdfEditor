// Mode Éditer : assemble la session d'édition, la couche posée sur les
// pages, la barre d'outils et le panneau Propriétés ; touches propres au
// mode (Suppr, flèches, Entrée, Échap) ; images choisies, collées ou
// déposées.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke, invokeBytes } from "../../lib/ipc.js";
import { EditLayer } from "./edit-layer.js";
import { EditSession } from "./edit-session.js";
import { duplicated, nudged, pagePoint, withStyle } from "./edit-state.js";
import { newImageBox } from "./image-state.js";
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
    byId("tool-image").addEventListener("click", () => this.pickImage());
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
    if (item?.element.kind === "text") this.session.change(item.id, withStyle(item.element, changes)).catch(fail);
    else this.styles.refresh();
  }

  /** Page et point (points) au centre de la partie visible de la page courante. */
  visibleCenter() {
    const index = this.viewer.page;
    return this.targetAt(index, null);
  }

  /**
   * Point de la page `index` sous (x, y) écran, ou centre de sa partie
   * visible si `client` est absent.
   */
  targetAt(index, client) {
    const frame = this.viewer.frames[index];
    const page = this.viewer.doc.pages[index];
    const r = frame.getBoundingClientRect();
    const d = this.viewer.desk.getBoundingClientRect();
    const x = client ? client.x : (Math.max(r.left, d.left) + Math.min(r.right, d.right)) / 2;
    const y = client ? client.y : (Math.max(r.top, d.top) + Math.min(r.bottom, d.bottom)) / 2;
    return { index, point: pagePoint(x - r.left, y - r.top, r.width, r.height, page) };
  }

  /** Pose une image en cours d'import (`null` : import annulé). */
  async insertImage(importing, target) {
    try {
      const imported = await importing;
      if (!imported || !this.viewer.doc) return;
      const at = target ?? this.visibleCenter();
      const page = this.viewer.doc.pages[at.index];
      await this.session.add(newImageBox(at.index, imported, page, at.point));
    } catch (error) {
      toast(t("edit.imageFailed", { message: error?.message ?? String(error) }), "error");
    }
  }

  pickImage() {
    this.layer.commitText();
    return this.insertImage(invoke("pick_image", { id: this.session.doc.id }));
  }

  /** Image du presse-papiers (fichier du navigateur). */
  pasteImage(file) {
    const bytes = file.arrayBuffer().then((buffer) =>
      invokeBytes("import_image_bytes", new Uint8Array(buffer), { "x-document": String(this.session.doc.id) }),
    );
    return this.insertImage(bytes);
  }

  /** Fichier déposé ; (x, y) écran : posé à cet endroit s'il tombe sur une page. */
  dropImage(path, client) {
    const target = client && document.elementFromPoint(client.x, client.y)?.closest(".page");
    const index = target ? this.viewer.frames.indexOf(target) : -1;
    const at = index >= 0 ? this.targetAt(index, client) : undefined;
    return this.insertImage(invoke("import_image_file", { id: this.session.doc.id, path }), at);
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
