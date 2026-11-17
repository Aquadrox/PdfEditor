// Panneau Propriétés de l'élément sélectionné (maquette Main.dc.html) :
// texte, ou opacité, rotation et proportions d'une image ; position et
// taille en mm ; dupliquer, supprimer. Les contrôles de style du panneau
// sont gérés par style-controls.js.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { isAnnotation } from "./annotation-state.js";
import { samePage } from "./selection-state.js";
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

/** Champs propres au genre d'annotation (`data-annot-only` : genres, ou « fillable »). */
function showAnnotationFields(element) {
  const fillable = element.kind === "shape" && (element.shape === "rectangle" || element.shape === "ellipse");
  for (const field of byId("prop-annot-fields").querySelectorAll("[data-annot-only]")) {
    const kinds = field.dataset.annotOnly.split(" ");
    field.hidden = !(kinds.includes(element.kind) || (fillable && kinds.includes("fillable")));
  }
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
  const noteText = byId("prop-note");
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
  noteText.addEventListener("change", () => {
    const item = selected();
    if (item?.element.kind === "note" && noteText.value !== item.element.text) {
      actions.change(item.id, { ...item.element, text: noteText.value });
    }
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
  byId("prop-duplicate").addEventListener("click", () => selected() && actions.duplicate());
  byId("prop-delete").addEventListener("click", () => selected() && actions.removeSelection());
  for (const button of document.querySelectorAll("[data-arrange]")) {
    button.addEventListener("click", () => actions.arrange(button.dataset.arrange));
  }
  byId("prop-close").addEventListener("click", actions.close);

  /**
   * Visible pendant tout le mode Éditer (la page ne change pas de zoom à
   * chaque sélection) ; sans sélection, une aide remplace les champs.
   */
  function refresh(visible = true) {
    const item = selected();
    const count = session.selection.length;
    // Plusieurs éléments : seulement aligner, répartir, dupliquer, supprimer.
    const many = count > 1;
    const kind = many ? "many" : item?.element.kind;
    panel.hidden = !visible;
    byId("prop-empty").hidden = Boolean(item);
    byId("prop-fields").hidden = !item;
    byId("prop-close").hidden = !item;
    byId("prop-arrange-fields").hidden = !many;
    byId("prop-geometry").hidden = many;
    if (many) {
      const items = session.selection.map((id) => session.element(id)).filter(Boolean);
      const together = samePage(items);
      byId("prop-arrange-note").hidden = together;
      for (const button of document.querySelectorAll("[data-arrange]")) {
        button.disabled = !together || (button.dataset.arrange.startsWith("distribute") && items.length < 3);
      }
    }
    byId("prop-text-fields").hidden = kind !== "text";
    byId("prop-image-fields").hidden = kind !== "image";
    byId("prop-note-fields").hidden = kind !== "note";
    if (kind === "note") {
      if (document.activeElement !== noteText) noteText.value = item.element.text;
      byId("prop-note-author").textContent = item.element.author ? t("props.noteAuthor", { author: item.element.author }) : t("props.noteNoAuthor");
    }
    byId("prop-annot-fields").hidden = many || !isAnnotation(item?.element);
    if (!many && isAnnotation(item?.element)) showAnnotationFields(item.element);
    const titles = { image: "props.imageTitle", shape: "props.shapeTitle", ink: "props.inkTitle", markup: "props.markupTitle", note: "props.noteTitle" };
    byId("properties-title").textContent = many ? t("props.manyTitle", { count }) : t(!item ? "props.panel" : (titles[kind] ?? "props.title"));
    if (!visible || !item || many) return;
    const values = geometryInMm(item.element, item.bounds);
    if (kind === "text" && document.activeElement !== text) text.value = item.element.text;
    // Caractères qu'aucune police embarquée ne couvre (pde-fonts).
    const missing = byId("prop-missing");
    missing.hidden = !item.missing_chars;
    missing.textContent = item.missing_chars ? t("props.missingChars", { chars: [...item.missing_chars].join(" ") }) : "";
    // Hauteur d'un texte : donnée par ses lignes ; taille d'un marquage :
    // celle du texte marqué. Ni l'une ni l'autre ne se saisit.
    geometry.height.readOnly = kind === "text" || kind === "markup" || kind === "note";
    geometry.width.readOnly = kind === "markup" || kind === "note";
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
