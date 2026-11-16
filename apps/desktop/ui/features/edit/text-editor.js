// Saisie directe d'un texte sur la page : un champ posé à l'emplacement de
// la zone, au corps et dans la police approchante. Ctrl+Entrée ou un clic
// ailleurs valide, Échap annule.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { percentBox } from "../viewer/text-geometry.js";
import { cssFontFamily, hexFromColor } from "./edit-state.js";

/**
 * Ouvre l'éditeur dans `layer` pour `element` (coordonnées de la page).
 * `onDone(text | null)` : texte validé, ou null si annulé.
 * Renvoie { commit, cancel } pour le fermer depuis l'extérieur.
 */
export function openTextEditor(layer, element, page, onDone) {
  const { style } = element;
  const area = el("textarea", { class: "text-editor", spellcheck: "true", "aria-label": t("props.text") });
  area.value = element.text;
  Object.assign(area.style, percentBox({ ...element.top_left, width: element.width, height: 0 }, page), {
    height: "auto",
    fontSize: `calc(var(--scale) * ${style.size}px)`,
    lineHeight: String(style.line_height),
    fontFamily: cssFontFamily(style.font),
    fontWeight: style.bold ? "700" : "400",
    fontStyle: style.italic ? "italic" : "normal",
    textAlign: style.align,
    color: hexFromColor(style.color),
  });
  let finished = false;
  const finish = (text) => {
    if (finished) return;
    finished = true;
    area.remove();
    onDone(text);
  };
  const grow = () => {
    area.style.height = "auto";
    area.style.height = `${area.scrollHeight}px`;
  };
  area.addEventListener("input", grow);
  // Les raccourcis de l'application (Ctrl+S…) restent actifs : app.js
  // laisse Ctrl+Z / Ctrl+Y au champ.
  area.addEventListener("keydown", (event) => {
    if (event.key === "Escape") finish(null);
    else if (event.key === "Enter" && (event.ctrlKey || event.metaKey)) finish(area.value);
  });
  area.addEventListener("blur", () => finish(area.value));
  area.addEventListener("pointerdown", (event) => event.stopPropagation());
  layer.append(area);
  grow();
  area.focus({ preventScroll: true });
  return { cancel: () => finish(null), commit: () => finish(area.value) };
}
