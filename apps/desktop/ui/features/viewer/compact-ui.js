// Téléphone (A-01) : assemble ce qui ne sert qu'au doigt ou à la
// disposition compacte (tiroir, feuilles, gestes, textes tactiles, Retour
// d'Android), pour que app.js n'ait qu'à signaler document, mode et Réglages.

import { onBack } from "../../lib/back.js";
import { isTouch } from "../../lib/compact.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { attachSidebarDrawer } from "./drawer.js";
import { attachSheets } from "./sheets.js";
import { attachTouchGestures } from "./touch-gestures.js";

/** Modes où un double tapotement zoome (ailleurs, la page s'édite). */
const DOUBLE_TAP_MODES = ["read", "sign", "ai"];

/**
 * @param {import("./viewer.js").Viewer} viewer
 * @param {() => string} currentMode mode affiché
 */
export function attachCompactUi(viewer, currentMode) {
  const app = document.querySelector(".app");
  // Au doigt : pas de fichier à déposer, pas de double-clic.
  if (isTouch()) {
    document.querySelector(".drop-title").textContent = t("home.titleTouch");
    document.querySelector(".pages-hint").textContent = t("pages.hintTouch");
  }
  const drawer = attachSidebarDrawer();
  attachSheets();
  attachTouchGestures(viewer, () => DOUBLE_TAP_MODES.includes(currentMode()));
  let releaseSettings = null;

  return {
    /** Document affiché ou non : barre des modes et bouton du tiroir. */
    setDocument(open) {
      drawer.setDocument(open);
      app.classList.toggle("has-document", open);
    },
    setMode(mode) {
      app.dataset.mode = mode;
    },
    /** Réglages ouverts : le Retour d'Android les ferme comme leur bouton « Retour ». */
    settingsOpened() {
      releaseSettings ??= onBack(() => {
        releaseSettings = null;
        byId("settings-back").click();
      });
    },
    settingsClosed() {
      releaseSettings?.();
      releaseSettings = null;
    },
  };
}
