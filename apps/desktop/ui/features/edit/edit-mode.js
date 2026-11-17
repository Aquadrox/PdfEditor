// Mode Éditer : assemble la session d'édition, la couche posée sur les
// pages, la barre d'outils (outils et leurs options) et le panneau
// Propriétés ; touches propres au mode (Suppr, flèches, Entrée, Échap) ;
// images choisies, collées ou déposées ; marquage du texte sélectionné.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke, invokeBytes } from "../../lib/ipc.js";
import { attachAnnotationControls, renderAnnotationSwatches } from "./annotation-controls.js";
import { TOOL_DEFAULTS, isAnnotation, withAnnotationStyle } from "./annotation-state.js";
import { colorFromHex, ptFromMm } from "./edit-state.js";
import { EditLayer } from "./edit-layer.js";
import { EditSession } from "./edit-session.js";
import { duplicated, nudged, pagePoint, withStyle } from "./edit-state.js";
import { newImageBox } from "./image-state.js";
import { aligned, distributed, nextInOrder, samePage } from "./selection-state.js";
import { attachMarkupTool } from "./markup-tool.js";
import { attachRedactTool } from "./redact-tool.js";
import { attachProperties, renderSwatches } from "./properties.js";
import { attachStyleControls } from "./style-controls.js";

const fail = (error) => toast(t("edit.failed", { message: error?.message ?? String(error) }), "error");

/** Outils qui se choisissent (l'image est une action) ; annotations : style par outil. */
const TOOLS = ["text", "markup", "ink", "shape", "note", "redact"];

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
    this.tool = "text";
    this.toolStyles = structuredClone(TOOL_DEFAULTS);
    this.layer.toolStyle = () => this.toolStyles[this.tool];
    /** Auteur des notes (réglage Édition). */
    this.author = "";
    this.layer.onNote = (note) => this.addNote(note);
    this.layer.onNoteOpen = () => byId("prop-note").focus();
    /** Magnétisme (réglage Édition). */
    this.snapSettings = { enabled: true, grid: 0 };
    this.layer.snap = () => this.snapSettings;
    const pages = () => viewer.doc?.pages ?? [];
    renderSwatches();
    renderAnnotationSwatches(document);
    // Barre d'outils : style de l'outil en cours (et de l'annotation
    // sélectionnée du même genre) ; panneau : annotation sélectionnée.
    this.toolControls = attachAnnotationControls(
      byId("edit-toolbar"),
      () => this.toolStyles[this.tool] ?? null,
      (changes) => this.applyToolStyle(changes),
    );
    this.annotationControls = attachAnnotationControls(
      byId("properties"),
      () => this.selectedAnnotation()?.element ?? null,
      (changes) => this.applyAnnotationStyle(changes),
    );
    this.styles = attachStyleControls(
      document,
      () => this.session.element(this.session.selected)?.element.style ?? this.session.style,
      (changes) => this.applyStyle(changes),
    );
    this.properties = attachProperties(this.session, pages, {
      change: (id, element) => this.session.change(id, element).catch(fail),
      remove: (id) => this.session.remove(id).catch(fail),
      // Dupliquer : toute la sélection, en une opération.
      duplicate: () => {
        const copies = this.selectedItems().map((i) => duplicated(i.element, pages()[i.element.page]));
        const done = copies.length === 1 ? this.session.add(copies[0]) : this.session.addMany(copies);
        done.catch(fail);
      },
      removeSelection: () => this.session.removeMany(this.session.selection).catch(fail),
      arrange: (action) => this.arrange(action),
      close: () => this.session.select(null),
    });
    for (const tool of TOOLS) byId(`tool-${tool}`).addEventListener("click", () => this.setTool(tool));
    // Outil Surligner : le texte glissé devient un marquage au relâchement.
    attachMarkupTool(
      viewer,
      () => this.active && this.tool === "markup",
      () => this.toolStyles.markup,
      (markup) => this.session.add(markup).catch(fail),
    );
    // Outil Caviarder : texte sélectionné ou zone tracée.
    attachRedactTool(
      viewer,
      () => this.active && this.tool === "redact",
      (redaction) => this.session.add(redaction).catch(fail),
    );
    byId("tool-image").addEventListener("click", () => this.pickImage());
    this.session.subscribe((view, changed) => {
      if (changed.length > 0) viewer.setRevisions(view.revisions, changed);
      this.layer.refresh();
      this.styles.refresh();
      this.properties.refresh(this.active);
      this.toolControls.refresh();
      this.annotationControls.refresh();
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

  /** Outil choisi : ses options dans la barre, son curseur sur la page. */
  setTool(tool) {
    this.layer.commitText();
    this.tool = tool;
    this.layer.tool = tool;
    for (const name of TOOLS) {
      const button = byId(`tool-${name}`);
      button.setAttribute("aria-pressed", String(name === tool));
      button.classList.toggle("on", name === tool);
      button.classList.toggle("ghost", name !== tool);
      this.viewer.desk.classList.toggle(`tool-${name}`, name === tool);
    }
    for (const group of document.querySelectorAll("[data-tool-options]")) {
      group.hidden = group.dataset.toolOptions !== tool;
    }
    this.toolControls.refresh();
  }

  /** Note posée : auteur du réglage, commentaire à écrire tout de suite. */
  async addNote(note) {
    try {
      await this.session.add({ ...note, author: this.author });
      byId("prop-note").focus();
    } catch (error) {
      fail(error);
    }
  }

  /** Réglages Édition : style des prochaines zones de texte, surlignage, auteur. */
  applySettings(settings) {
    if (!settings) return;
    this.author = settings.author ?? "";
    const text = colorFromHex(settings.text_color);
    this.session.style = {
      ...this.session.style,
      font: settings.font,
      size: settings.font_size,
      ...(text && { color: text }),
    };
    this.snapSettings = { enabled: settings.snap !== false, grid: ptFromMm(settings.grid_mm ?? 0) };
    const highlight = colorFromHex(settings.highlight_color);
    if (highlight) this.toolStyles.markup.color = highlight;
    this.styles.refresh();
    this.toolControls.refresh();
  }

  /** Annotation sélectionnée qui a un style (une zone caviardée n'en a pas). */
  selectedAnnotation() {
    const item = this.session.element(this.session.selected);
    return item && isAnnotation(item.element) && item.element.kind !== "redact" ? item : null;
  }

  /** Option de l'outil en cours ; s'applique aussi à l'annotation sélectionnée du même genre. */
  applyToolStyle(changes) {
    const style = this.toolStyles[this.tool];
    if (!style) return;
    Object.assign(style, withAnnotationStyle({ kind: this.tool, ...style }, changes));
    delete style.kind;
    const item = this.selectedAnnotation();
    if (item?.element.kind === this.tool) this.session.change(item.id, withAnnotationStyle(item.element, changes)).catch(fail);
    else this.toolControls.refresh();
  }

  /** Style changé dans le panneau : l'annotation, et les prochaines du même genre. */
  applyAnnotationStyle(changes) {
    const item = this.selectedAnnotation();
    if (!item) return;
    const kind = item.element.kind;
    const style = this.toolStyles[kind];
    Object.assign(style, withAnnotationStyle({ kind, ...style }, changes));
    delete style.kind;
    this.session.change(item.id, withAnnotationStyle(item.element, changes)).catch(fail);
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
  /**
   * Touches du mode. Sur un bouton, seul Ctrl+A est pris : Tab, Entrée,
   * flèches gardent leur rôle (passer au contrôle suivant, l'activer).
   */
  handleKey(event, { onButton = false } = {}) {
    if (!this.active || !this.viewer.doc) return false;
    // Ctrl+A : tous les éléments de la page affichée (jamais tout le texte
    // de la fenêtre).
    if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
      const ids = this.session.view.elements.filter((e) => e.element.page === this.viewer.page).map((e) => e.id);
      this.session.selectMany(ids);
      return true;
    }
    if (onButton) return false;
    // Tab / Maj+Tab : élément suivant ou précédent (ordre de lecture).
    if (event.key === "Tab" && !event.ctrlKey && !event.altKey) {
      const next = nextInOrder(this.session.view.elements, this.session.selected, event.shiftKey);
      if (next === null) return false;
      this.session.select(next);
      this.revealSelected();
      return true;
    }
    const items = this.selectedItems();
    if (items.length === 0) return false;
    if (event.key === "Delete" || event.key === "Backspace") {
      this.session.removeMany(items.map((i) => i.id)).catch(fail);
    } else if (event.key === "Escape") {
      this.session.select(null);
    } else if (event.key === "Enter") {
      if (items.length === 1) this.layer.editSelected();
    } else {
      const pages = this.viewer.doc.pages;
      const changes = items.map((i) => ({ id: i.id, element: nudged(i.element, event.key, event.shiftKey, pages[i.element.page]) }));
      if (changes.some((c) => !c.element)) return false;
      this.session.changeMany(changes).catch(fail);
    }
    return true;
  }

  /** Éléments sélectionnés, l'élément principal d'abord. */
  selectedItems() {
    return this.session.selection.map((id) => this.session.element(id)).filter(Boolean);
  }

  /** Page de l'élément sélectionné amenée à l'écran (Tab). */
  revealSelected() {
    const item = this.session.element(this.session.selected);
    if (item && item.element.page !== this.viewer.page) this.viewer.goToPage(item.element.page);
  }

  /** Aligner ou répartir la sélection (même page). */
  arrange(action) {
    const items = this.selectedItems();
    if (items.length < 2 || !samePage(items)) return;
    const page = this.viewer.doc.pages[items[0].element.page];
    const changes = action === "distribute-x" || action === "distribute-y"
      ? distributed(items, action.slice(-1), page)
      : aligned(items, action, page);
    this.session.changeMany(changes).catch(fail);
  }

}
