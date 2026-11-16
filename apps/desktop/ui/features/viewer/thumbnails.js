// Miniatures des pages dans le panneau de gauche ; images chargées à
// l'approche de la zone visible du panneau.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { pageUrl } from "../../lib/ipc.js";
import { CSS_PX_PER_POINT } from "./zoom.js";
import { renderScale } from "./layout.js";

/** Largeur d'une miniature (px CSS). */
const THUMB_WIDTH = 96;

export class Thumbnails {
  constructor(list, scrollRoot, onSelect) {
    this.list = list;
    this.onSelect = onSelect;
    this.buttons = [];
    this.revisions = [];
    this.observer = new IntersectionObserver((entries) => this.onVisible(entries), {
      root: scrollRoot,
      rootMargin: "300px 0px",
    });
  }

  load(doc) {
    this.clear();
    this.doc = doc;
    this.buttons = doc.pages.map((page, index) => {
      const zoom = THUMB_WIDTH / (page.width * CSS_PX_PER_POINT);
      const image = el("img", {
        class: "thumb-image",
        alt: "",
        width: THUMB_WIDTH,
        height: Math.round(page.height * CSS_PX_PER_POINT * zoom),
        "data-page": index,
        "data-zoom": zoom,
      });
      const button = el(
        "button",
        { class: "thumb", "aria-label": t("thumb.label", { n: index + 1 }), onclick: () => this.onSelect(index) },
        image,
        el("span", { class: "num" }, String(index + 1)),
      );
      this.observer.observe(image);
      return button;
    });
    this.list.append(...this.buttons);
    this.setCurrent(0);
  }

  clear() {
    this.observer.disconnect();
    this.list.replaceChildren();
    this.buttons = [];
    this.revisions = [];
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
      const image = this.buttons[index]?.querySelector("img");
      if (image?.src) this.observer.observe(image);
    }
  }

  setCurrent(index) {
    this.buttons.forEach((button, i) => {
      if (i === index) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    });
    this.buttons[index]?.scrollIntoView({ block: "nearest" });
  }
}
