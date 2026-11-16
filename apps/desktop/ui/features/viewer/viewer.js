// Visionneuse : pages sur la table lumineuse, défilement continu, zoom.
// Seules les pages proches de la zone visible ont une image ; les calculs
// de position sont dans layout.js (testés), ce module ne fait que le DOM.

import { pageUrl } from "../../lib/ipc.js";
import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import {
  anchorAt,
  computeLayout,
  currentPage,
  renderScale,
  scrollTopForAnchor,
  scrollTopForPage,
  visibleRange,
} from "./layout.js";
import { clampZoom, fitPage, fitWidth } from "./zoom.js";

/** Délai avant de redemander les images après un changement de zoom (ms). */
const RERENDER_DELAY = 150;

export class Viewer {
  /**
   * @param {HTMLElement} desk zone qui défile
   * @param {{ onPageChange?: (page: number) => void, onZoomChange?: (zoom: number) => void }} callbacks
   */
  constructor(desk, callbacks = {}) {
    this.desk = desk;
    this.callbacks = callbacks;
    this.container = el("div", { class: "pages" });
    this.desk.append(this.container);
    this.doc = null;
    this.zoom = 1;
    this.fitMode = "width";
    this.page = 0;
    this.frames = [];
    this.layout = computeLayout([], 1);
    this.rerenderTimer = 0;
    this.scheduled = false;

    this.desk.addEventListener("scroll", () => this.schedule(), { passive: true });
    new ResizeObserver(() => this.onResize()).observe(this.desk);
  }

  /** Affiche un document (DocumentView de pde-app). */
  load(doc) {
    this.unload();
    this.doc = doc;
    this.frames = doc.pages.map((_, index) =>
      el("div", { class: "page", role: "img", "aria-label": t("thumb.label", { n: index + 1 }) }),
    );
    this.container.append(...this.frames);
    this.fitMode = "width";
    this.applyZoom(this.fittedZoom() ?? 1, { page: 0, fraction: 0 });
    this.desk.scrollTop = 0;
  }

  unload() {
    this.doc = null;
    this.frames = [];
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
    // Au-delà de trois écrans, l'image est libérée (mémoire du WebView).
    const keep = visibleRange(this.layout, scrollTop, clientHeight, 3 * clientHeight);
    this.frames.forEach((frame, index) => {
      if (near && index >= near.first && index <= near.last) this.showImage(frame, index);
      else if (!keep || index < keep.first || index > keep.last) frame.replaceChildren();
    });
  }

  showImage(frame, index) {
    const scale = renderScale(this.doc.pages[index], this.zoom, window.devicePixelRatio || 1);
    const current = frame.querySelector("img");
    if (current && Number(current.dataset.scale) === scale) return;
    // L'ancienne image reste affichée (étirée) jusqu'à l'arrivée de la nouvelle.
    const image = el("img", { alt: "", decoding: "async", "data-scale": scale });
    image.addEventListener("load", () => {
      frame.classList.remove("failed");
      frame.replaceChildren(image);
    });
    image.addEventListener("error", () => {
      frame.classList.add("failed");
      frame.dataset.error = t("page.error");
    });
    image.src = pageUrl(this.doc.id, index, scale);
    if (!current) frame.replaceChildren(image);
  }
}
