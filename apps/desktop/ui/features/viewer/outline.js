// Plan du document (signets) : liste imbriquée, clic → page.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

/** Remplit `container` avec le plan ; renvoie false s'il n'y en a pas. */
export function renderOutline(container, outline, onSelect) {
  if (outline.length === 0) {
    container.replaceChildren(el("p", { class: "outline-empty" }, t("outline.empty")));
    return false;
  }
  container.replaceChildren(list(outline, onSelect, 0));
  return true;
}

function list(items, onSelect, depth) {
  return el("ul", {}, ...items.map((item) => entry(item, onSelect, depth)));
}

function entry(item, onSelect, depth) {
  const button = el(
    "button",
    {
      class: "outline-item",
      disabled: item.page === null,
      onclick: (event) => {
        event.preventDefault();
        onSelect(item.page);
      },
    },
    item.title || "—",
  );
  if (item.children.length === 0) return el("li", {}, button);
  // Premier niveau déplié, les suivants repliés.
  return el(
    "li",
    {},
    el("details", { open: depth === 0 }, el("summary", {}, button), list(item.children, onSelect, depth + 1)),
  );
}
