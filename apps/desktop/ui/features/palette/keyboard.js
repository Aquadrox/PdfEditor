// Raccourcis clavier de l'application (R-08) : touches des modes Pages et
// Éditer d'abord, puis les actions réglables (palette, enregistrer, zoom…).

import { byId } from "../../lib/dom.js";
import { eventToShortcut } from "../../lib/shortcuts.js";
import { isEditable } from "../viewer/desk-input.js";

/**
 * @param {object} app
 * @param {{ handleKey(event: KeyboardEvent): boolean }} app.pages
 * @param {{ handleKey(event: KeyboardEvent, options: object): boolean }} app.edit
 * @param {() => Map<string, string>} app.keymap raccourci → action
 * @param {Record<string, () => void>} app.actions
 */
export function attachKeyboard({ pages, edit, keymap, actions }) {
  document.addEventListener("keydown", (event) => {
    // Page Réglages ou dialogue ouvert : seuls leurs propres contrôles réagissent.
    if (!byId("settings-view").hidden || document.querySelector("dialog[open]")) return;
    const editable = isEditable(event.target);
    // Mode Pages : Suppr, Ctrl+A, flèches, Entrée sur la grille.
    if (!editable && pages.handleKey(event)) return event.preventDefault();
    // Suppr, flèches, Entrée, Échap : sur la zone sélectionnée, pas sur un bouton.
    if (!editable && edit.handleKey(event, { onButton: event.target instanceof HTMLButtonElement })) {
      return event.preventDefault();
    }
    const shortcut = eventToShortcut(event);
    const action = shortcut && keymap().get(shortcut);
    if (!action || (editable && (action === "undo" || action === "redo"))) return;
    event.preventDefault();
    actions[action]?.();
  });
}
