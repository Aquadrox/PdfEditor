// Barre latérale : onglets Pages (miniatures) et Plan (signets).

import { byId } from "../../lib/dom.js";

/** Affiche l'onglet `tab` ("pages" ou "outline"). */
export function showSidebarTab(tab) {
  const outline = tab === "outline";
  for (const [id, selected] of [["tab-pages", !outline], ["tab-outline", outline]]) {
    byId(id).classList.toggle("on", selected);
    byId(id).classList.toggle("ghost", !selected);
    byId(id).setAttribute("aria-selected", String(selected));
  }
  byId("thumbs").hidden = outline;
  byId("outline").hidden = !outline;
}

byId("tab-pages").addEventListener("click", () => showSidebarTab("pages"));
byId("tab-outline").addEventListener("click", () => showSidebarTab("outline"));
