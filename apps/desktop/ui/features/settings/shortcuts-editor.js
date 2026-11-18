// Tableau des raccourcis : un clic sur un raccourci attend la nouvelle
// combinaison ; Échap annule. Les doublons sont refusés tout de suite.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { eventToShortcut, isAcceptable, shortcutLabel } from "../../lib/shortcuts.js";

/** Ordre d'affichage (celui de pde-core). */
const ACTIONS = [
  "open",
  "save",
  "save_as",
  "close",
  "undo",
  "redo",
  "search",
  "zoom_in",
  "zoom_out",
  "zoom_reset",
  "fit_page",
  "fit_width",
  "settings",
  "palette",
  "how_to",
];

/**
 * @param {HTMLTableSectionElement} body corps du tableau
 * @param {() => Record<string, string>} current raccourcis du brouillon
 * @param {(action: string, shortcut: string) => void} onChange
 */
export function renderShortcuts(body, current, onChange) {
  const shortcuts = current();
  body.replaceChildren(
    ...ACTIONS.map((action) => {
      const message = el("span", { class: "field-error", role: "alert" });
      const button = el(
        "button",
        { class: "btn shortcut-button", type: "button", onclick: () => capture(button, message, action) },
        shortcutLabel(shortcuts[action] ?? ""),
      );
      return el("tr", {}, el("th", { scope: "row" }, t(`action.${action}`)), el("td", {}, button, message));
    }),
  );

  function capture(button, message, action) {
    button.textContent = t("settings.shortcuts.press");
    button.classList.add("capturing");
    message.textContent = "";
    const stop = () => {
      window.removeEventListener("keydown", onKey, true);
      button.classList.remove("capturing");
      button.textContent = shortcutLabel(current()[action] ?? "");
    };
    // Phase de capture : la touche n'atteint pas les raccourcis de l'application.
    const onKey = (event) => {
      event.preventDefault();
      event.stopPropagation();
      if (event.key === "Escape") return stop();
      const shortcut = eventToShortcut(event);
      if (!shortcut) return; // modificateur seul : on attend la suite
      const other = Object.entries(current()).find(([a, s]) => s === shortcut && a !== action);
      if (!isAcceptable(shortcut)) message.textContent = t("settings.shortcuts.invalid");
      else if (other) message.textContent = t("settings.shortcuts.taken", { action: t(`action.${other[0]}`) });
      else onChange(action, shortcut);
      stop();
    };
    window.addEventListener("keydown", onKey, true);
  }
}
