// Visionneuse : pages sur la table lumineuse, défilement continu, zoom,
// couche de texte et surlignage de la recherche. Seules les pages proches de
// la zone visible ont une image et une couche de texte ; les calculs de
// position sont dans layout.js et text-geometry.js (testés), ce module ne
// fait que le DOM.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { pageUrl } from "../../lib/ipc.js";
import {
  anchorAt,
  computeLayout,
  currentPage,
  renderScale,
  scrollTopForAnchor,
  scrollTopForPage,
  visibleRange,
} from "./layout.js";
import { highlightLayer, textLayer } from "./page-layers.js";
import { scrollTopForRect } from "./text-geometry.js";
import { CSS_PX_PER_POINT, clampZoom, fitModeFor, fitPage, fitWidth } from "./zoom.js";

/** Délai avant de redemander les images après un changement de zoom (ms). */
const RERENDER_DELAY = 150;

export class Viewer {
  /**
   * @param {HTMLElement} desk zone qui défile
   * @param {{
   *   onPageChange?: (page: number) => void,
   *   onZoomChange?: (zoom: number) => void,
   *   loadText?: (page: number) => Promise<{ runs: object[] }>,
   *   onPageShown?: (frame: HTMLElement, page: number) => void,
   * }} callbacks
   */
  constructor(desk, callbacks = {}) {
    this.desk = desk;
    this.callbacks = callbacks;
    this.container = el("div", { class: "pages" });
    this.desk.append(this.container);
    this.doc = null;
    this.zoom = 1;
    this.fitMode = "width";
    /** Réglage display.default_zoom : "fit_width", "fit_page" ou "actual". */
    this.defaultZoom = "fit_width";
    this.page = 0;
    this.frames = [];
    this.layout = computeLayout([], 1);
    this.rerenderTimer = 0;
    this.scheduled = false;
    /** Texte des pages déjà reçu (promesses), pour reconstruire les couches. */
    this.texts = new Map();
    this.hits = [];
    this.currentHit = -1;
    /** Version du contenu de chaque page (édition), mise dans l'adresse des images. */
    this.revisions = [];

    this.desk.addEventListener("scroll", () => this.schedule(), { passive: true });
    new ResizeObserver(() => this.onResize()).observe(this.desk);
  }

  /** Affiche un document (DocumentView de pde-app). */
  load(doc) {
    this.setDocument(doc);
    // Zoom d'ouverture (réglage display.default_zoom).
    this.fitMode = fitModeFor(this.defaultZoom);
    this.applyZoom(this.fittedZoom() ?? 1, { page: 0, fraction: 0 });
    this.desk.scrollTop = 0;
  }

  /**
   * Même document, pages changées (mode Pages) : zoom gardé, affichage
   * ramené à `page`. Les images suivent les nouvelles versions (setRevisions).
   */
  reload(doc, page = 0) {
    const { fitMode, zoom } = this;
    this.setDocument(doc);
    this.fitMode = fitMode;
    this.applyZoom(this.fittedZoom() ?? zoom, { page: Math.min(page, doc.pages.length - 1), fraction: 0 });
  }

  setDocument(doc) {
    this.unload();
    this.doc = doc;
    this.frames = doc.pages.map((_, index) =>
      el("div", { class: "page", role: "group", "aria-label": t("thumb.label", { n: index + 1 }) }),
    );
    this.container.append(...this.frames);
  }

  unload() {
    this.doc = null;
    this.frames = [];
    this.texts.clear();
    this.hits = [];
    this.currentHit = -1;
    this.revisions = [];
    this.container.replaceChildren();
    this.page = 0;
  }

  /** Zoom choisi par l'utilisateur : l'ajustement automatique s'arrête. */
  setZoom(zoom) {
    this.fitMode = null;
    this.applyZoom(clampZoom(zoom));
  }

  /** "width" : largeur de la page ; "page" : page entière. */
  fit(mode) {
    this.fitMode = mode;
    const zoom = this.fittedZoom();
    if (zoom !== null) this.applyZoom(zoom, mode === "page" ? { page: this.page, fraction: 0 } : undefined);
    if (mode === "page") this.goToPage(this.page);
  }

  goToPage(index) {
    if (!this.doc) return;
    this.desk.scrollTop = scrollTopForPage(this.layout, index);
    this.schedule();
  }

  /**
   * Résultats de recherche à surligner (SearchHit de pde-app) ; `current` :
   * occurrence active, amenée à l'écran.
   */
  setHighlights(hits, current = -1) {
    this.hits = hits;
    this.currentHit = current;
    const hit = hits[current];
    if (hit && this.doc) {
      const pageBox = this.layout.boxes[hit.page];
      this.desk.scrollTop = scrollTopForRect(pageBox, this.doc.pages[hit.page], hit.rects[0], this.desk.clientHeight);
    }
    this.frames.forEach((frame) => frame.querySelector(".highlights")?.remove());
    this.schedule();
  }

  /**
   * Pages modifiées par l'édition : nouvelle image (l'ancienne reste
   * affichée jusqu'à son arrivée) et texte relu.
   */
  setRevisions(revisions, changed) {
    this.revisions = revisions;
    for (const index of changed) {
      this.texts.delete(index);
      const frame = this.frames[index];
      if (!frame) continue;
      frame.querySelector(".text-layer")?.remove();
      const image = frame.querySelector("img");
      if (image) image.dataset.scale = "";
    }
    this.schedule();
  }

  fittedZoom() {
    if (!this.doc || this.doc.pages.length === 0) return null;
    const { clientWidth, clientHeight } = this.desk;
    if (this.fitMode === "width") return fitWidth(this.doc.pages, clientWidth);
    if (this.fitMode === "page") return fitPage(this.doc.pages[this.page], clientWidth, clientHeight);
    return null;
  }

  onResize() {
    const zoom = this.fittedZoom();
    if (zoom !== null && Math.abs(zoom - this.zoom) > 0.001) this.applyZoom(zoom);
    else this.schedule();
  }

  /** Change le zoom en gardant le même endroit du document à l'écran. */
  applyZoom(zoom, anchor = anchorAt(this.layout, this.desk.scrollTop)) {
    if (!this.doc) return;
    this.zoom = zoom;
    this.layout = computeLayout(this.doc.pages, zoom);
    // Échelle lue par la couche de texte pour le corps des caractères.
    this.container.style.setProperty("--scale", String(zoom * CSS_PX_PER_POINT));
    this.container.style.height = `${this.layout.totalHeight}px`;
    this.container.style.width = `${this.layout.maxWidth + 48}px`;
    this.layout.boxes.forEach((box, index) => {
      const frame = this.frames[index];
      frame.style.top = `${box.top}px`;
      frame.style.width = `${box.width}px`;
      frame.style.height = `${box.height}px`;
    });
    this.desk.scrollTop = scrollTopForAnchor(this.layout, anchor);
    this.callbacks.onZoomChange?.(zoom);
    // Les images actuelles sont étirées tout de suite ; les nettes suivent.
    clearTimeout(this.rerenderTimer);
    this.rerenderTimer = setTimeout(() => this.update(), RERENDER_DELAY);
    this.schedule();
  }

  /** Mise à jour au plus une fois par image affichée par l'écran. */
  schedule() {
    if (this.scheduled) return;
    this.scheduled = true;
    requestAnimationFrame(() => {
      this.scheduled = false;
      this.update();
    });
  }

  update() {
    if (!this.doc) return;
    const { scrollTop, clientHeight } = this.desk;
    const page = currentPage(this.layout, scrollTop, clientHeight);
    if (page !== this.page) {
      this.page = page;
      this.callbacks.onPageChange?.(page);
    }
    const near = visibleRange(this.layout, scrollTop, clientHeight);
    // Au-delà de trois écrans, la page est vidée (mémoire du WebView).
    const keep = visibleRange(this.layout, scrollTop, clientHeight, 3 * clientHeight);
    this.frames.forEach((frame, index) => {
      if (near && index >= near.first && index <= near.last) this.showPage(frame, index);
      else if (!keep || index < keep.first || index > keep.last) frame.replaceChildren();
    });
  }

  showPage(frame, index) {
    this.showImage(frame, index);
    this.showText(frame, index);
    this.showHighlights(frame, index);
    this.callbacks.onPageShown?.(frame, index);
  }

  showImage(frame, index) {
    const scale = renderScale(this.doc.pages[index], this.zoom, window.devicePixelRatio || 1);
    const current = frame.querySelector("img");
    if (current && Number(current.dataset.scale) === scale) return;
    // L'ancienne image reste affichée (étirée) jusqu'à l'arrivée de la nouvelle.
    const image = el("img", { alt: "", decoding: "async", "data-scale": scale });
    image.addEventListener("load", () => {
      frame.classList.remove("failed");
      const previous = frame.querySelector("img");
      if (previous && previous !== image) previous.replaceWith(image);
    });
    image.addEventListener("error", () => {
      frame.classList.add("failed");
      frame.dataset.error = t("page.error");
    });
    image.src = pageUrl(this.doc.id, index, scale, this.revisions[index]);
    if (!current) frame.prepend(image);
  }

  showText(frame, index) {
    if (!this.callbacks.loadText || frame.querySelector(".text-layer") || frame.dataset.textPending) return;
    if (!this.texts.has(index)) this.texts.set(index, this.callbacks.loadText(index));
    const doc = this.doc;
    frame.dataset.textPending = "1";
    this.texts
      .get(index)
      .then((view) => {
        // Document changé ou page vidée entre-temps : rien à poser.
        if (this.doc !== doc || !frame.isConnected || frame.querySelector(".text-layer")) return;
        const layer = textLayer(view, doc.pages[index]);
        // Sous les surlignages et la couche d'édition.
        const above = frame.querySelector(".highlights, .edit-layer");
        if (above) above.before(layer);
        else frame.append(layer);
      })
      .catch(() => this.texts.delete(index))
      .finally(() => delete frame.dataset.textPending);
  }

  showHighlights(frame, index) {
    if (frame.querySelector(".highlights")) return;
    const hits = this.hits.map((hit, i) => ({ hit, index: i })).filter(({ hit }) => hit.page === index);
    if (hits.length === 0) return;
    const layer = highlightLayer(hits, this.doc.pages[index], this.currentHit);
    const editLayer = frame.querySelector(".edit-layer");
    if (editLayer) editLayer.before(layer);
    else frame.append(layer);
  }
}
