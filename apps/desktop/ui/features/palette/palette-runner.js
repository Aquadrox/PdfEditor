// Exécution des actions du catalogue depuis la palette : par leur raccourci
// s'il y en a un, sinon en actionnant leur outil dans la barre. « Montrer »
// passe dans le bon mode et met l'outil en évidence (« Comment faire ? »).

import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

/** Durée de la mise en évidence d'un outil (ms). */
const SPOTLIGHT_MS = 2400;

/**
 * @param {object} app
 * @param {() => boolean} app.hasDocument
 * @param {() => string} app.mode mode affiché (read, edit, pages, sign, ai)
 * @param {(mode: string) => void} app.setMode
 * @param {Record<string, () => void>} app.shortcuts actions des raccourcis
 */
export function createRunner(app) {
  const toolOf = (action) => document.querySelector(action.tool);

  /** Raison pour laquelle l'action n'est pas possible maintenant, ou null. */
  function unavailable(action) {
    if (action.needs_document && !app.hasDocument()) return t("palette.needsDocument");
    if (action.mode && byId(`mode-${action.mode}`).disabled) return t("palette.locked");
    const tool = toolOf(action);
    if (!tool) return t("palette.unavailable");
    // Un raccourci sait quoi faire même si son bouton est grisé (Annuler en mode Pages…).
    if (tool.disabled && !action.shortcut) return t("palette.unavailable");
    return null;
  }

  /** Passe dans le mode de l'outil, s'il le faut. */
  function enterMode(action) {
    if (action.mode && app.mode() !== action.mode) app.setMode(action.mode);
  }

  function run(action) {
    if (unavailable(action)) return false;
    if (action.shortcut && app.shortcuts[action.shortcut]) {
      app.shortcuts[action.shortcut]();
      return true;
    }
    enterMode(action);
    const tool = toolOf(action);
    if (tool instanceof HTMLInputElement || tool instanceof HTMLTextAreaElement) {
      tool.focus();
      tool.select?.();
    } else {
      tool.click();
    }
    return true;
  }

  /** Montre où se trouve l'outil ; renvoie faux s'il n'est pas atteignable. */
  function show(action) {
    if (unavailable(action) === t("palette.needsDocument")) return false;
    enterMode(action);
    const tool = toolOf(action);
    if (!tool) return false;
    const menu = tool.closest("#main-menu");
    if (menu?.hidden) byId("menu-button").click();
    tool.scrollIntoView({ block: "nearest", inline: "nearest" });
    tool.classList.remove("spotlight");
    // Relance l'animation si l'outil était déjà mis en évidence.
    void tool.offsetWidth;
    tool.classList.add("spotlight");
    setTimeout(() => tool.classList.remove("spotlight"), SPOTLIGHT_MS);
    tool.focus({ preventScroll: true });
    return true;
  }

  return { unavailable, run, show };
}
