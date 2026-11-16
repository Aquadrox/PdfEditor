// Barre d'état : page courante (saisissable), zoom et ajustements.

import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { formatZoom, zoomIn, zoomOut } from "./zoom.js";

export function attachStatusbar(viewer) {
  const input = byId("page-input");
  const count = byId("page-count");
  const value = byId("zoom-value");

  input.addEventListener("change", () => {
    const page = Number.parseInt(input.value, 10);
    if (viewer.doc && page >= 1 && page <= viewer.doc.pages.length) viewer.goToPage(page - 1);
    else input.value = String(viewer.page + 1);
  });
  byId("zoom-in").addEventListener("click", () => viewer.setZoom(zoomIn(viewer.zoom)));
  byId("zoom-out").addEventListener("click", () => viewer.setZoom(zoomOut(viewer.zoom)));
  byId("fit-width").addEventListener("click", () => viewer.fit("width"));
  byId("fit-page").addEventListener("click", () => viewer.fit("page"));

  return {
    setDocument(doc) {
      byId("statusbar").hidden = !doc;
      if (!doc) return;
      input.max = String(doc.pages.length);
      count.textContent = t("status.of", { count: doc.pages.length });
    },
    setPage(page) {
      if (document.activeElement !== input) input.value = String(page + 1);
    },
    setZoom(zoom) {
      value.textContent = formatZoom(zoom);
    },
  };
}
