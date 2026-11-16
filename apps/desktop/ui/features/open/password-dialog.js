// Demande du mot de passe d'un document protégé (V-02).

import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

let wired = false;

function wire(dialog) {
  if (wired) return;
  wired = true;
  byId("password-cancel").addEventListener("click", () => dialog.close("cancel"));
}

/** Mot de passe saisi, ou null si l'utilisateur annule. */
export function askPassword(name, wrongBefore) {
  const dialog = byId("password-dialog");
  const input = byId("password-input");
  wire(dialog);
  const error = byId("password-error");
  byId("password-text").textContent = t("password.text", { name });
  error.hidden = !wrongBefore;
  input.setAttribute("aria-invalid", wrongBefore ? "true" : "false");
  input.value = "";
  return new Promise((resolve) => {
    dialog.addEventListener(
      "close",
      () => {
        const password = dialog.returnValue === "ok" ? input.value : null;
        input.value = "";
        resolve(password);
      },
      { once: true },
    );
    dialog.returnValue = "";
    dialog.showModal();
    input.focus();
  });
}
