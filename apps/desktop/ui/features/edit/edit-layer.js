// Couche d'édition posée sur chaque page : contour des éléments ajoutés,
// sélection, déplacement et redimensionnement à la souris (bords gauche et
// droit d'une zone de texte, coins d'une image), création d'une zone de
// texte par un clic avec l'outil Texte.

import { toast } from "../../components/toast.js";
import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { percentBox } from "../viewer/text-geometry.js";
import { moved, newTextBox, pagePoint, pointsFromPixels, resized, withText } from "./edit-state.js";
import { resizedFrame } from "./image-state.js";
import { openTextEditor } from "./text-editor.js";

/** En dessous (pixels), un appui sur un élément est un clic, pas un déplacement. */
const DRAG_THRESHOLD = 3;

function fail(error) {
  toast(t("edit.failed", { message: error?.message ?? String(error) }), "error");
}

/** Poignées : largeur d'une zone de texte, coins d'une image. */
const HANDLES = { text: ["left", "right"], image: ["nw", "ne", "sw", "se"] };

/** Élément après un redimensionnement de (dx, dy) points par la poignée `edge`. */
function resizedBy(element, edge, dx, dy, page) {
  return element.kind === "image" ? resizedFrame(element, edge, dx, dy, page) : resized(element, edge, dx, page);
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
    const boxes = this.session.view.elements
      .filter((e) => e.element.page === index && e.id !== this.editing?.id)
      .map((item) => this.box(item, page, index));
    layer.querySelectorAll(".element").forEach((node) => node.remove());
    layer.prepend(...boxes);
  }

  box(item, page, index) {
    const selected = item.id === this.session.selected;
    const box = el("div", {
      class: selected ? "element selected" : "element",
      "data-id": item.id,
      role: "button",
      "aria-label": t(item.element.kind === "image" ? "edit.imageBox" : "edit.textBox"),
      "aria-pressed": selected ? "true" : "false",
    });
    Object.assign(box.style, percentBox(item.bounds, page));
    if (this.active) {
      box.addEventListener("pointerdown", (event) => this.onElementDown(event, box, item, page, null));
      if (item.element.kind === "text") {
        box.addEventListener("dblclick", () => this.editText(box.parentElement, item.id, index));
      }
      if (selected) {
        for (const edge of HANDLES[item.element.kind]) {
          const handle = el("span", { class: `handle ${edge}`, "aria-hidden": "true" });
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
    if (this.tool !== "text") return;
    const page = this.viewer.doc.pages[index];
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

  /** Appui sur un élément : sélection, puis déplacement ou redimensionnement. */
  onElementDown(event, box, item, page, edge) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    if (this.session.selected !== item.id) this.session.select(item.id);
    const frameWidth = box.parentElement.getBoundingClientRect().width;
    const start = { x: event.clientX, y: event.clientY };
    let dragged = false;
    const move = (e) => {
      const dx = e.clientX - start.x;
      const dy = e.clientY - start.y;
      dragged ||= Math.hypot(dx, dy) > DRAG_THRESHOLD;
      const target = box.isConnected ? box : this.findBox(item.id);
      if (dragged && target) target.style.transform = edge ? "" : `translate(${dx}px, ${dy}px)`;
      if (dragged && target && edge) Object.assign(target.style, this.resizedStyle(item, page, edge, dx, dy, frameWidth));
    };
    const up = async (e) => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
      if (!dragged) return;
      const { dx, dy } = pointsFromPixels(e.clientX - start.x, e.clientY - start.y, frameWidth, page);
      const element = edge ? resizedBy(item.element, edge, dx, dy, page) : moved(item.element, dx, dy, page);
      await this.session.change(item.id, element).catch((error) => this.failed(error));
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  /** Aperçu pendant le glissement ; le moteur donne ensuite la place exacte. */
  resizedStyle(item, page, edge, dxPx, dyPx, frameWidth) {
    const { dx, dy } = pointsFromPixels(dxPx, dyPx, frameWidth, page);
    const element = resizedBy(item.element, edge, dx, dy, page);
    const height = element.kind === "image" ? element.height : item.bounds.height;
    return percentBox({ ...element.top_left, width: element.width, height }, page);
  }

  findBox(id) {
    return document.querySelector(`.element[data-id="${id}"]`);
  }

  /** Entrée sur la zone sélectionnée : modifier son texte. */
  editSelected() {
    const item = this.session.element(this.session.selected);
    if (item?.element.kind !== "text") return;
    const frame = this.viewer.frames[item.element.page];
    if (frame && layerOf(frame)) this.editText(layerOf(frame), item.id, item.element.page);
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
