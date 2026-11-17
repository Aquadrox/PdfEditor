// Couche d'édition posée sur chaque page : contour des éléments ajoutés,
// sélection, déplacement et redimensionnement à la souris (bords gauche et
// droit d'une zone de texte, coins d'une image ou d'un cadre, extrémités
// d'un trait), création d'une zone de texte (outil Texte) ou d'un tracé
// (outils Dessin et Forme, voir draw-tool.js).

import { toast } from "../../components/toast.js";
import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { percentBox } from "../viewer/text-geometry.js";
import { moved, newTextBox, pagePoint, pointsFromPixels, resized, withText } from "./edit-state.js";
import { annotationResized, bboxOf, handlesOf, isAnnotation, newNote } from "./annotation-state.js";
import { startDrawing } from "./draw-tool.js";
import { snapped, union } from "./selection-state.js";
import { resizedFrame } from "./image-state.js";
import { asImage, isExisting, withFrame } from "./existing-image.js";
import { editorBox, isOriginalText, withNewText } from "./original-text.js";
import { openTextEditor } from "./text-editor.js";

/** En dessous (pixels), un appui sur un élément est un clic, pas un déplacement. */
const DRAG_THRESHOLD = 3;
/** Distance (pixels à l'écran) à laquelle le magnétisme attire un repère. */
const SNAP_PX = 6;

function fail(error) {
  toast(t("edit.failed", { message: error?.message ?? String(error) }), "error");
}

/** Poignées : largeur d'une zone de texte, coins d'une image, voir handlesOf. */
function handles(element) {
  if (element.kind === "text") return ["left", "right"];
  // Le texte d'origine garde sa place et sa taille.
  if (isOriginalText(element)) return [];
  if (element.kind === "image" || isExisting(element)) return ["nw", "ne", "sw", "se"];
  return handlesOf(element);
}

/** Élément après un redimensionnement de (dx, dy) points par la poignée `edge`. */
function resizedBy(element, edge, dx, dy, page) {
  if (isAnnotation(element)) return annotationResized(element, edge, dx, dy, page);
  if (isExisting(element)) return withFrame(element, resizedFrame(asImage(element), edge, dx, dy, page));
  return element.kind === "image" ? resizedFrame(element, edge, dx, dy, page) : resized(element, edge, dx, page);
}

/** Libellé lu par les lecteurs d'écran pour un élément. */
function labelOf(element) {
  if (element.kind === "shape") return `edit.shape.${element.shape}`;
  return {
    text: "edit.textBox",
    image: "edit.imageBox",
    ink: "edit.inkBox",
    note: "edit.noteBox",
    redact: "edit.redactBox",
    existing_image: "edit.existingImage",
    original_text: "edit.originalText",
    markup: `edit.mark.${element.mark}`,
  }[element.kind];
}

/** Le moteur arrive en premier dans un cadre vidé : couche au-dessus de tout. */
function layerOf(frame) {
  return frame.querySelector(".edit-layer");
}

export class EditLayer {
  /** @param {import("./edit-session.js").EditSession} session */
  constructor(session, viewer) {
    this.session = session;
    this.viewer = viewer;
    this.active = false;
    this.tool = "text";
    /** Style de l'outil Dessin ou Forme en cours (donné par EditMode). */
    this.toolStyle = () => null;
    /** Magnétisme (réglage Édition) : { enabled, grid } en points. */
    this.snap = () => ({ enabled: true, grid: 0 });
    /** Note posée : EditMode l'ajoute et ouvre son commentaire. */
    this.onNote = () => {};
    /** Double-clic sur une note : son commentaire dans le panneau. */
    this.onNoteOpen = () => {};
    this.editing = null;
  }

  /** Mode Éditer : la couche reçoit la souris, le texte n'est plus sélectionnable. */
  setActive(active) {
    this.active = active;
    this.editing?.commit();
    this.refresh();
  }

  /** Redessine les pages affichées (les autres le seront à leur arrivée). */
  refresh() {
    this.viewer.frames.forEach((frame, index) => {
      if (frame.querySelector("img")) this.render(frame, index);
    });
  }

  /** Page affichée par la visionneuse : sa couche, si elle ne l'a pas. */
  pageShown(frame, index) {
    if (!layerOf(frame)) this.render(frame, index);
  }

  /** Valide la saisie en cours (avant d'enregistrer, de changer de mode…). */
  commitText() {
    this.editing?.commit();
  }

  /** (Re)dessine la couche d'une page. */
  render(frame, index) {
    if (!this.viewer.doc) return;
    let layer = layerOf(frame);
    if (!layer) {
      layer = el("div", { class: "edit-layer" });
      layer.addEventListener("pointerdown", (event) => this.onLayerDown(event, layer, index));
      frame.append(layer);
    }
    layer.classList.toggle("active", this.active);
    const page = this.viewer.doc.pages[index];
    // Images d'origine de la page : demandées une fois, la couche est
    // redessinée à leur arrivée.
    const redraw = (loaded) => loaded && this.render(frame, index);
    if (this.active) this.session.loadPageImages(index).then(redraw, () => {});
    // Outil Texte du document : les lignes de la page se proposent.
    const retext = this.active && this.tool === "retext";
    if (retext) this.session.loadPageTextBlocks(index).then(redraw, () => {});
    const boxes = this.session
      .itemsOn(index)
      .filter((e) => e.id !== this.editing?.id)
      .map((item) => this.box(item, page, index));
    const blocks = retext ? this.session.blocksOn(index).filter((b) => b.id !== this.editing?.id).map((b) => this.blockBox(b, page, index)) : [];
    layer.querySelectorAll(".element").forEach((node) => node.remove());
    layer.prepend(...boxes, ...blocks);
  }

  /** Ligne de texte d'origine proposée : un clic l'ouvre à la saisie. */
  blockBox(item, page, index) {
    const box = el("div", { class: "element text-block", "data-id": item.id, role: "button", "aria-label": t("edit.originalBlock") });
    Object.assign(box.style, percentBox(item.bounds, page));
    box.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      event.preventDefault();
      event.stopPropagation();
      this.editOriginal(box.parentElement, item, index);
    });
    return box;
  }

  box(item, page, index) {
    const selected = this.session.isSelected(item.id);
    // Poignées pour un seul élément sélectionné.
    const single = selected && this.session.selection.length === 1;
    const box = el("div", {
      class: selected ? "element selected" : "element",
      "data-id": item.id,
      role: "button",
      "aria-label": t(labelOf(item.element)),
      "aria-pressed": selected ? "true" : "false",
    });
    Object.assign(box.style, percentBox(item.bounds, page));
    if (this.active) {
      box.addEventListener("pointerdown", (event) => this.onElementDown(event, box, item, page, null));
      if (item.element.kind === "text") {
        box.addEventListener("dblclick", () => this.editText(box.parentElement, item.id, index));
      }
      if (item.element.kind === "note") box.addEventListener("dblclick", () => this.onNoteOpen());
      if (isOriginalText(item.element)) {
        box.addEventListener("dblclick", () => this.editOriginal(box.parentElement, item, index));
      }
      if (single) {
        for (const edge of handles(item.element)) {
          const handle = el("span", { class: `handle ${edge}`, "aria-hidden": "true" });
          if (edge === "start" || edge === "end") this.placeOnPoint(handle, item, edge);
          handle.addEventListener("pointerdown", (event) => this.onElementDown(event, box, item, page, edge));
          box.append(handle);
        }
      }
    }
    return box;
  }

  onLayerDown(event, layer, index) {
    if (!this.active || event.target !== layer || event.button !== 0) return;
    event.preventDefault();
    // Un clic à côté d'une saisie en cours la valide, sans en ouvrir une autre.
    if (this.editing) return this.editing.commit();
    this.session.select(null);
    const page = this.viewer.doc.pages[index];
    if (this.tool === "note") {
      const rect = layer.getBoundingClientRect();
      const point = pagePoint(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height, page);
      this.onNote(newNote(index, point, page, this.toolStyle(), ""));
      return;
    }
    if (this.tool === "ink" || this.tool === "shape") {
      startDrawing(event, layer, index, page, this.tool, this.toolStyle(), (element) => {
        if (element) this.session.add(element).catch(fail);
      });
      return;
    }
    if (this.tool !== "text") return;
    const rect = layer.getBoundingClientRect();
    const point = pagePoint(event.clientX - rect.left, event.clientY - rect.top, rect.width, rect.height, page);
    const element = newTextBox(index, point, page, this.session.style);
    this.editing = openTextEditor(layer, element, page, async (text) => {
      this.editing = null;
      if (!text?.trim()) return;
      await this.session.add(withText(element, text)).catch(fail);
    });
  }

  /** Échec côté moteur : on remet les éléments à leur place affichée. */
  failed(error) {
    fail(error);
    this.refresh();
  }

  /**
   * Appui sur un élément : Maj+clic l'ajoute à la sélection (ou l'en
   * retire) ; sinon sélection, puis déplacement de toute la sélection, ou
   * redimensionnement par une poignée.
   */
  onElementDown(event, box, item, page, edge) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    if (!edge && event.shiftKey) return this.session.toggle(item.id);
    // Couche mémorisée avant de sélectionner : la sélection redessine les
    // contours, et `box` n'est alors plus dans la page.
    const layer = box.parentElement;
    if (!this.session.isSelected(item.id)) this.session.select(item.id);
    // Le texte d'origine reste à sa place : sélection seulement.
    if (isOriginalText(item.element)) return;
    if (edge) return this.resizeFrom(event, layer, item, page, edge);
    return this.moveSelection(event, layer, item, page);
  }

  resizeFrom(event, layer, item, page, edge) {
    const frameWidth = layer.getBoundingClientRect().width;
    const start = { x: event.clientX, y: event.clientY };
    let dragged = false;
    const move = (e) => {
      const dx = e.clientX - start.x;
      const dy = e.clientY - start.y;
      dragged ||= Math.hypot(dx, dy) > DRAG_THRESHOLD;
      const target = this.findBox(item.id);
      if (dragged && target) Object.assign(target.style, this.resizedStyle(item, page, edge, dx, dy, frameWidth));
    };
    const up = async (e) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (!dragged) return;
      const { dx, dy } = pointsFromPixels(e.clientX - start.x, e.clientY - start.y, frameWidth, page);
      await this.session.change(item.id, resizedBy(item.element, edge, dx, dy, page)).catch((error) => this.failed(error));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /**
   * Glisser : toute la sélection suit, calée par le magnétisme (bords et
   * centres des autres éléments et de la page, grille) sauf avec Alt.
   */
  moveSelection(event, layer, item, page) {
    const ratio = page.width / layer.getBoundingClientRect().width;
    const group = this.session.selection
      .map((id) => this.session.element(id))
      .filter((g) => g && !isOriginalText(g.element));
    const pageIndex = item.element.page;
    const frame = union(group.filter((g) => g.element.page === pageIndex).map((g) => g.bounds));
    const others = this.session.view.elements
      .filter((e) => e.element.page === pageIndex && !this.session.isSelected(e.id))
      .map((e) => e.bounds);
    const start = { x: event.clientX, y: event.clientY };
    let dragged = false;
    let offset = { dx: 0, dy: 0 };
    const move = (e) => {
      const px = e.clientX - start.x;
      const py = e.clientY - start.y;
      dragged ||= Math.hypot(px, py) > DRAG_THRESHOLD;
      if (!dragged) return;
      const snap = this.snap();
      const raw = { dx: px * ratio, dy: py * ratio, guides: [] };
      const result = snap.enabled && !e.altKey ? snapped(frame, raw.dx, raw.dy, others, page, SNAP_PX * ratio, snap.grid) : raw;
      offset = result;
      for (const g of group) {
        const target = this.findBox(g.id);
        if (target) target.style.transform = `translate(${result.dx / ratio}px, ${result.dy / ratio}px)`;
      }
      this.showGuides(layer, result.guides, page);
    };
    const up = async () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      this.showGuides(layer, [], page);
      if (!dragged) return;
      const pages = this.viewer.doc.pages;
      const changes = group.map((g) => ({ id: g.id, element: moved(g.element, offset.dx, offset.dy, pages[g.element.page]) }));
      await this.session.changeMany(changes).catch((error) => this.failed(error));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /** Repères du magnétisme : une ligne par axe calé. */
  showGuides(layer, guides, page) {
    layer.querySelectorAll(".snap-guide").forEach((node) => node.remove());
    for (const { axis, at } of guides) {
      const line = el("div", { class: `snap-guide ${axis}`, "aria-hidden": "true" });
      if (axis === "x") line.style.left = `${(at / page.width) * 100}%`;
      else line.style.top = `${(at / page.height) * 100}%`;
      layer.append(line);
    }
  }

  /** Aperçu pendant le glissement ; le moteur donne ensuite la place exacte. */
  resizedStyle(item, page, edge, dxPx, dyPx, frameWidth) {
    const { dx, dy } = pointsFromPixels(dxPx, dyPx, frameWidth, page);
    const element = resizedBy(item.element, edge, dx, dy, page);
    if (isAnnotation(element)) return percentBox(bboxOf(element), page);
    const shown = isExisting(element) ? asImage(element) : element;
    const height = shown.kind === "image" ? shown.height : item.bounds.height;
    return percentBox({ ...shown.top_left, width: shown.width, height }, page);
  }

  /** Poignée d'extrémité d'un trait, posée sur son point dans le contour. */
  placeOnPoint(handle, item, edge) {
    const p = item.element[edge];
    const { x, y, width, height } = item.bounds;
    handle.style.left = `${((p.x - x) / width) * 100}%`;
    handle.style.top = `${((p.y - y) / height) * 100}%`;
  }

  findBox(id) {
    return document.querySelector(`.element[data-id="${id}"]`);
  }

  /** Entrée sur la zone sélectionnée : modifier son texte. */
  editSelected() {
    const item = this.session.element(this.session.selected);
    const kind = item?.element.kind;
    const frame = item && this.viewer.frames[item.element.page];
    if (!frame || !layerOf(frame)) return;
    if (kind === "text") this.editText(layerOf(frame), item.id, item.element.page);
    else if (isOriginalText(item.element)) this.editOriginal(layerOf(frame), item, item.element.page);
  }

  /**
   * Saisie d'une ligne de texte d'origine, à sa place (fond opaque : le
   * texte d'origine ne se voit pas dessous). Bloc pas encore modifié : son
   * élément est créé à la validation, seulement si le texte change.
   */
  editOriginal(layer, item, index) {
    if (!layer) return;
    this.editing?.commit();
    const page = this.viewer.doc.pages[index];
    const fresh = typeof item.id === "string";
    const editor = openTextEditor(layer, editorBox(item.element, page), page, async (text) => {
      this.editing = null;
      const element = text === null ? null : withNewText(item.element, text);
      if (!element || element.text === item.element.text) return this.render(layer.parentElement, index);
      const done = fresh ? this.session.add(element) : this.session.change(item.id, element);
      return done.catch((error) => this.failed(error));
    }, { opaque: true, singleLine: true });
    this.editing = { ...editor, id: item.id };
    this.render(layer.parentElement, index);
  }

  /** Double-clic ou Entrée : modifier le texte d'une zone existante. */
  editText(layer, id, index) {
    const item = this.session.element(id);
    if (!item || !layer) return;
    this.editing?.commit();
    const page = this.viewer.doc.pages[index];
    const editor = openTextEditor(layer, item.element, page, async (text) => {
      this.editing = null;
      if (text === null || text === item.element.text) return this.render(layer.parentElement, index);
      if (!text.trim()) return this.session.remove(id).catch((error) => this.failed(error));
      return this.session.change(id, withText(item.element, text)).catch((error) => this.failed(error));
    });
    this.editing = { ...editor, id };
    this.render(layer.parentElement, index);
  }
}
