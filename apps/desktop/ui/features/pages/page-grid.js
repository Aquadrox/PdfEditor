// Mode Pages : grille des pages du document. Clic, Ctrl+clic, Maj+clic pour
// choisir ; glisser pour réordonner (événements de pointeur : le glisser-
// déposer HTML est intercepté par la fenêtre pour les fichiers déposés) ;
// double-clic pour ouvrir la page. Images chargées à l'approche de la zone
// visible. Les calculs de sélection et de dépôt sont dans page-selection.js.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { pageUrl } from "../../lib/ipc.js";
import { renderScale } from "../viewer/layout.js";
import { CSS_PX_PER_POINT } from "../viewer/zoom.js";
import { clicked, dropIndex, isStill, selectAll, selectOnly } from "./page-selection.js";

/** Cadre de l'image d'une carte (px CSS) : la page y tient, proportions gardées. */
const SHEET_WIDTH = 152;
const SHEET_HEIGHT = 200;
/** Déplacement du pointeur (px) au-delà duquel un appui devient un glisser. */
const DRAG_THRESHOLD = 6;
/** Bande (px) près des bords où la grille défile pendant un glisser. */
const SCROLL_EDGE = 48;

export class PageGrid {
  /**
   * @param {HTMLElement} root zone qui défile
   * @param {{
   *   onSelection: (selection: object) => void,
   *   onMove: (pages: number[], to: number) => void,
   *   onOpen: (page: number) => void,
   * }} callbacks
   */
  constructor(root, callbacks) {
    this.root = root;
    this.callbacks = callbacks;
    this.list = el("div", { class: "page-grid-list", role: "listbox", "aria-multiselectable": "true" });
    this.list.setAttribute("aria-label", t("pages.grid"));
    this.marker = el("div", { class: "drop-marker", hidden: true });
    this.root.append(this.list, this.marker);
    this.doc = null;
    this.cards = [];
    this.revisions = [];
    this.selection = selectOnly(0);
    this.press = null;
    this.observer = new IntersectionObserver((entries) => this.onVisible(entries), {
      root: this.root,
      rootMargin: "400px 0px",
    });
    this.root.addEventListener("pointermove", (event) => this.onPointerMove(event));
    this.root.addEventListener("pointerup", (event) => this.onPointerUp(event));
    this.root.addEventListener("pointercancel", () => this.endDrag());
  }

  /** Affiche les pages de `doc` ; la position de défilement est gardée. */
  load(doc, revisions) {
    const scrollTop = this.root.scrollTop;
    this.observer.disconnect();
    this.doc = doc;
    this.revisions = revisions;
    this.cards = doc.pages.map((page, index) => this.card(page, index));
    this.list.replaceChildren(...this.cards);
    this.root.scrollTop = scrollTop;
    this.render();
  }

  clear() {
    this.observer.disconnect();
    this.doc = null;
    this.cards = [];
    this.list.replaceChildren();
  }

  card(page, index) {
    const fit = Math.min(SHEET_WIDTH / page.width, SHEET_HEIGHT / page.height);
    const image = el("img", {
      class: "page-card-image",
      alt: "",
      draggable: "false",
      width: Math.round(page.width * fit),
      height: Math.round(page.height * fit),
      "data-page": index,
      "data-zoom": fit / CSS_PX_PER_POINT,
    });
    const card = el(
      "div",
      {
        class: "page-card",
        role: "option",
        "aria-selected": "false",
        "aria-label": t("thumb.label", { n: index + 1 }),
        "data-page": index,
        onpointerdown: (event) => this.onPointerDown(event, index),
        ondblclick: () => this.callbacks.onOpen(index),
      },
      el("div", { class: "page-card-sheet" }, image),
      el("span", { class: "page-card-num num" }, String(index + 1)),
    );
    this.observer.observe(image);
    return card;
  }

  onVisible(entries) {
    for (const entry of entries) {
      if (!entry.isIntersecting || !this.doc) continue;
      const image = entry.target;
      const index = Number(image.dataset.page);
      const scale = renderScale(this.doc.pages[index], Number(image.dataset.zoom), window.devicePixelRatio || 1);
      image.src = pageUrl(this.doc.id, index, scale, this.revisions[index]);
      this.observer.unobserve(image);
    }
  }

  /** Pages modifiées : nouvelle image si elle était déjà chargée. */
  setRevisions(revisions, changed) {
    this.revisions = revisions;
    for (const index of changed) {
      const image = this.cards[index]?.querySelector("img");
      if (image?.src) this.observer.observe(image);
    }
  }

  setSelection(selection) {
    this.selection = selection;
    this.render();
  }

  /** Nombre de colonnes de la grille (flèches haut et bas). */
  get columns() {
    const tops = this.cards.map((card) => card.offsetTop);
    const first = tops[0];
    const count = tops.filter((top) => top === first).length;
    return Math.max(1, count);
  }

  render() {
    const selected = new Set(this.selection.pages);
    this.cards.forEach((card, index) => {
      card.setAttribute("aria-selected", String(selected.has(index)));
      card.classList.toggle("focus", index === this.selection.focus);
    });
    this.cards[this.selection.focus]?.scrollIntoView({ block: "nearest" });
  }

  select(selection) {
    this.setSelection(selection);
    this.callbacks.onSelection(selection);
  }

  selectAll() {
    this.select(selectAll(this.cards.length));
  }

  onPointerDown(event, index) {
    if (event.button !== 0) return;
    this.root.focus({ preventScroll: true });
    this.press = {
      index,
      x: event.clientX,
      y: event.clientY,
      shift: event.shiftKey,
      toggle: event.ctrlKey || event.metaKey,
      dragging: false,
      target: null,
    };
  }

  onPointerMove(event) {
    const press = this.press;
    if (!press) return;
    if (!press.dragging) {
      if (Math.hypot(event.clientX - press.x, event.clientY - press.y) < DRAG_THRESHOLD) return;
      press.dragging = true;
      // Une page non choisie se glisse seule.
      if (!this.selection.pages.includes(press.index)) this.select(selectOnly(press.index));
      this.root.setPointerCapture?.(event.pointerId);
      this.root.classList.add("dragging");
    }
    this.autoScroll(event.clientY);
    const boxes = this.cards.map((card) => card.getBoundingClientRect());
    press.target = dropIndex(boxes, event.clientX, event.clientY);
    this.showMarker(press.target, boxes, event.clientY);
  }

  onPointerUp(event) {
    const press = this.press;
    if (!press) return;
    if (press.dragging) {
      const pages = this.selection.pages;
      const to = press.target;
      this.endDrag();
      if (to !== null && !isStill(pages, to)) this.callbacks.onMove(pages, to);
      return;
    }
    this.press = null;
    this.select(clicked(this.selection, press.index, { shift: press.shift, toggle: press.toggle }));
    event.preventDefault();
  }

  endDrag() {
    this.press = null;
    this.marker.hidden = true;
    this.root.classList.remove("dragging");
  }

  autoScroll(y) {
    const { top, bottom } = this.root.getBoundingClientRect();
    if (y < top + SCROLL_EDGE) this.root.scrollTop -= SCROLL_EDGE / 4;
    else if (y > bottom - SCROLL_EDGE) this.root.scrollTop += SCROLL_EDGE / 4;
  }

  /**
   * Trait vertical à la place de dépôt : devant la carte `index`, ou après
   * la précédente quand le pointeur est au bout de sa rangée.
   */
  showMarker(index, boxes, y) {
    const previous = boxes[index - 1];
    const afterPrevious = previous && (index === boxes.length || (y >= previous.top && y < previous.bottom));
    const box = afterPrevious ? previous : boxes[index];
    if (!box) return;
    const origin = this.root.getBoundingClientRect();
    const x = afterPrevious ? box.right + 6 : box.left - 6;
    this.marker.style.left = `${x - origin.left + this.root.scrollLeft}px`;
    this.marker.style.top = `${box.top - origin.top + this.root.scrollTop}px`;
    this.marker.style.height = `${box.height}px`;
    this.marker.hidden = false;
  }
}
