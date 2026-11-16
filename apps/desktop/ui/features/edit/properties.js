// Panneau Propriétés de l'élément sélectionné (maquette Main.dc.html) :
// texte, ou opacité, rotation et proportions d'une image ; position et
// taille en mm ; dupliquer, supprimer. Les contrôles de style du panneau
// sont gérés par style-controls.js.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { geometryInMm, SWATCHES, withGeometryMm, withText } from "./edit-state.js";
import { rotated, withOpacity } from "./image-state.js";

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
 * @param {() => object[]} pages
 * @param {{ change, duplicate, remove, close }} actions
 */
export function attachProperties(session, pages, actions) {
  const panel = byId("properties");
  const text = byId("prop-text");
  const geometry = {
    x: byId("prop-x"),
    y: byId("prop-y"),
    width: byId("prop-width"),
    height: byId("prop-height"),
  };
  const opacity = byId("prop-opacity");
  const opacityValue = byId("prop-opacity-value");
  const keepRatio = byId("prop-keep-ratio");
  const selected = () => session.element(session.selected);
  const pageOf = (item) => pages()[item.element.page];
  /** Modifie l'élément sélectionné si `make` renvoie un élément. */
  const changeSelected = (make) => {
    const item = selected();
    const element = item && make(item.element, pageOf(item));
    if (element) actions.change(item.id, element);
    else refresh();
  };

  text.addEventListener("change", () => {
    const item = selected();
    if (!item || text.value === item.element.text) return;
    if (text.value.trim()) actions.change(item.id, withText(item.element, text.value));
    else actions.remove(item.id);
  });
  for (const [field, input] of Object.entries(geometry)) {
    input.addEventListener("change", () => changeSelected((element, page) => withGeometryMm(element, field, input.value, page)));
  }
  opacity.addEventListener("input", () => {
    opacityValue.textContent = `${opacity.value} %`;
  });
  opacity.addEventListener("change", () => changeSelected((element) => withOpacity(element, opacity.value)));
  byId("prop-rotate-left").addEventListener("click", () => changeSelected((element, page) => rotated(element, -1, page)));
  byId("prop-rotate-right").addEventListener("click", () => changeSelected((element, page) => rotated(element, 1, page)));
  keepRatio.addEventListener("change", () => changeSelected((element) => ({ ...element, keep_ratio: keepRatio.checked })));
  byId("prop-duplicate").addEventListener("click", () => selected() && actions.duplicate(selected()));
  byId("prop-delete").addEventListener("click", () => selected() && actions.remove(selected().id));
  byId("prop-close").addEventListener("click", actions.close);

  /**
   * Visible pendant tout le mode Éditer (la page ne change pas de zoom à
   * chaque sélection) ; sans sélection, une aide remplace les champs.
   */
  function refresh(visible = true) {
    const item = selected();
    const kind = item?.element.kind;
    panel.hidden = !visible;
    byId("prop-empty").hidden = Boolean(item);
    byId("prop-fields").hidden = !item;
    byId("prop-close").hidden = !item;
    byId("prop-text-fields").hidden = kind !== "text";
    byId("prop-image-fields").hidden = kind !== "image";
    byId("properties-title").textContent = t(!item ? "props.panel" : kind === "image" ? "props.imageTitle" : "props.title");
    if (!visible || !item) return;
    const values = geometryInMm(item.element, item.bounds);
    if (kind === "text" && document.activeElement !== text) text.value = item.element.text;
    // Hauteur d'un texte : donnée par ses lignes, pas saisissable.
    geometry.height.readOnly = kind === "text";
    for (const [field, input] of Object.entries(geometry)) {
      if (document.activeElement !== input) input.value = String(values[field]).replace(".", ",");
    }
    if (kind === "image") {
      const percent = Math.round(item.element.opacity * 100);
      opacity.value = String(percent);
      opacityValue.textContent = `${percent} %`;
      keepRatio.checked = item.element.keep_ratio;
    }
  }

  return { refresh };
}
