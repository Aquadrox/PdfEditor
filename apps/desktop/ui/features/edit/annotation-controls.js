// Contrôles des annotations (barre d'outils et panneau Propriétés) : tout
// contrôle marqué `data-annot` montre le style de l'annotation visée (ou
// celui de l'outil) et le modifie.
//
//   <button data-annot="color" data-value="#rrggbb">  couleur en un clic
//   <input type="color" data-annot="color">          autre couleur
//   <input data-annot="width">                       épaisseur (pt)
//   <input type="checkbox" data-annot="filled">      remplissage
//   <button data-annot="shape|mark" data-value>      forme, marquage

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { ANNOTATION_COLORS } from "./annotation-state.js";
import { colorFromHex, hexFromColor } from "./edit-state.js";

export const annotationHex = hexFromColor;

/** Pastilles de couleur dans chaque `[data-annot-swatches]` de `root`. */
export function renderAnnotationSwatches(root) {
  for (const container of root.querySelectorAll("[data-annot-swatches]")) {
    const swatches = ANNOTATION_COLORS.map(({ name, hex }) => {
      const button = el("button", { class: "swatch", "data-annot": "color", "data-value": hex, "aria-label": t(`color.${name}`), title: t(`color.${name}`) });
      // Couleur du document (donnée), pas un jeton de l'interface.
      button.style.backgroundColor = hex;
      return button;
    });
    const other = el("input", { class: "color-input", type: "color", "data-annot": "color", "aria-label": t("edit.otherColor"), title: t("edit.otherColor") });
    container.replaceChildren(...swatches, other);
  }
}

/** Changement demandé par un contrôle ; null si la saisie est invalide. */
function changeFrom(control) {
  const name = control.dataset.annot;
  if (name === "color") {
    const color = colorFromHex(control.dataset.value ?? control.value);
    return color && { color };
  }
  if (name === "width") {
    const width = Number.parseFloat(String(control.value).replace(",", "."));
    return Number.isFinite(width) ? { width: Math.min(Math.max(width, 0.25), 48) } : null;
  }
  if (name === "filled") return { filled: control.checked };
  return { [name]: control.dataset.value };
}

/**
 * @param {ParentNode} root
 * @param {() => object | null} current style affiché ({ color, width, filled, shape, mark })
 * @param {(changes: object) => void} apply
 */
export function attachAnnotationControls(root, current, apply) {
  const controls = () => [...root.querySelectorAll("[data-annot]")];
  root.addEventListener("click", (event) => {
    const control = event.target.closest?.("button[data-annot]");
    if (control && root.contains(control)) apply(changeFrom(control));
  });
  root.addEventListener("change", (event) => {
    const control = event.target;
    if (!control.matches?.("input[data-annot]")) return;
    const changes = changeFrom(control);
    if (changes) apply(changes);
    else refresh();
  });

  function refresh() {
    const style = current();
    if (!style) return;
    for (const control of controls()) {
      const name = control.dataset.annot;
      const value = name === "color" ? hexFromColor(style.color) : style[name];
      if (control.tagName === "BUTTON") {
        const pressed = control.dataset.value === value;
        control.setAttribute("aria-pressed", String(pressed));
        control.classList.toggle("on", pressed);
      } else if (control.type === "checkbox") {
        control.checked = Boolean(value);
      } else if (document.activeElement !== control && value !== undefined) {
        control.value = String(value);
      }
    }
  }

  return { refresh };
}
