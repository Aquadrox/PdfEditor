// Panneau Propriétés de la zone sélectionnée (maquette Main.dc.html) :
// texte, position et largeur en mm, dupliquer, supprimer. Les contrôles de
// style du panneau sont gérés par style-controls.js.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { geometryInMm, SWATCHES, withGeometryMm, withText } from "./edit-state.js";

/** Pastilles de couleur et « Autre couleur » ; avant attachStyleControls. */
export function renderSwatches() {
  const swatches = SWATCHES.map(({ name, hex }) => {
    const button = el("button", {
      class: "swatch",
      "data-style": "color",
      "data-value": hex,
      "aria-label": t(`color.${name}`),
      title: t(`color.${name}`),
    });
    // Couleur du document (donnée), pas un jeton de l'interface.
    button.style.backgroundColor = hex;
    return button;
  });
  const other = el("input", { class: "color-input", type: "color", "data-style": "color", "aria-label": t("edit.otherColor"), title: t("edit.otherColor") });
  byId("prop-swatches").replaceChildren(...swatches, other);
}

/**
 * @param {import("./edit-session.js").EditSession} session
 * @param {{ change, duplicate, remove, close }} actions
 */
export function attachProperties(session, pages, actions) {
  const panel = byId("properties");
  const text = byId("prop-text");
  const geometry = {
    x: byId("prop-x"),
    y: byId("prop-y"),
    width: byId("prop-width"),
  };
  const height = byId("prop-height");
  const selected = () => session.element(session.selected);

  text.addEventListener("change", () => {
    const item = selected();
    if (!item || text.value === item.element.text) return;
    if (text.value.trim()) actions.change(item.id, withText(item.element, text.value));
    else actions.remove(item.id);
  });
  for (const [field, input] of Object.entries(geometry)) {
    input.addEventListener("change", () => {
      const item = selected();
      if (!item) return;
      const element = withGeometryMm(item.element, field, input.value, pages()[item.element.page]);
      if (element) actions.change(item.id, element);
      else refresh();
    });
  }
  byId("prop-duplicate").addEventListener("click", () => selected() && actions.duplicate(selected()));
  byId("prop-delete").addEventListener("click", () => selected() && actions.remove(selected().id));
  byId("prop-close").addEventListener("click", actions.close);

  /**
   * Visible pendant tout le mode Éditer (la page ne change pas de zoom à
   * chaque sélection) ; sans sélection, une aide remplace les champs.
   */
  function refresh(visible = true) {
    const item = selected();
    panel.hidden = !visible;
    byId("prop-empty").hidden = Boolean(item);
    byId("prop-fields").hidden = !item;
    byId("prop-close").hidden = !item;
    byId("properties-title").textContent = t(item ? "props.title" : "props.panel");
    if (!visible || !item) return;
    const values = geometryInMm(item.element, item.bounds);
    if (document.activeElement !== text) text.value = item.element.text;
    for (const [field, input] of Object.entries(geometry)) {
      if (document.activeElement !== input) input.value = String(values[field]).replace(".", ",");
    }
    height.value = String(values.height).replace(".", ",");
  }

  return { refresh };
}
