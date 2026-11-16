// Contrôles de style (barre d'outils et panneau Propriétés) : tout contrôle
// marqué `data-style` montre le style de la zone sélectionnée, ou celui des
// prochaines zones, et le modifie.
//
//   <select data-style="font">              police
//   <input data-style="size">               corps (pt)
//   <button data-style="bold|italic">       bascule
//   <button data-style="align" data-value>  alignement
//   <button data-style="color" data-value>  couleur en un clic
//   <input type="color" data-style="color"> autre couleur

import { colorFromHex, hexFromColor, parseSize } from "./edit-state.js";

const TOGGLES = new Set(["bold", "italic"]);

function valueOf(control, style) {
  const name = control.dataset.style;
  if (name === "color") return hexFromColor(style.color);
  return style[name];
}

/** Changement demandé par un contrôle, ou null si la saisie est invalide. */
function changeFrom(control, style) {
  const name = control.dataset.style;
  if (TOGGLES.has(name)) return { [name]: !style[name] };
  if (name === "size") return parseSize(control.value) && { size: parseSize(control.value) };
  if (name === "color") {
    const color = colorFromHex(control.dataset.value ?? control.value);
    return color && { color };
  }
  if (name === "align") return { align: control.dataset.value };
  return { [name]: control.value };
}

/**
 * @param {ParentNode} root
 * @param {() => object} currentStyle style affiché
 * @param {(changes: object) => void} apply
 */
export function attachStyleControls(root, currentStyle, apply) {
  const controls = [...root.querySelectorAll("[data-style]")];
  for (const control of controls) {
    const event = control.tagName === "BUTTON" ? "click" : "change";
    control.addEventListener(event, () => {
      const changes = changeFrom(control, currentStyle());
      if (changes) apply(changes);
      else refresh();
    });
  }

  function refresh() {
    const style = currentStyle();
    for (const control of controls) {
      const value = valueOf(control, style);
      if (control.tagName === "BUTTON") {
        const pressed = TOGGLES.has(control.dataset.style) ? Boolean(value) : control.dataset.value === value;
        control.setAttribute("aria-pressed", String(pressed));
        control.classList.toggle("on", pressed);
      } else if (document.activeElement !== control) {
        control.value = String(value);
      }
    }
  }

  return { refresh };
}
