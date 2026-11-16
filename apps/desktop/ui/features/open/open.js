// Ouverture d'un fichier : mot de passe demandé si besoin, erreurs expliquées.

import { toast } from "../../components/toast.js";
import { fileName } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { askPassword } from "./password-dialog.js";

/** Ouvre `path` ; renvoie la description du document, ou null (annulé, erreur). */
export async function openPath(path) {
  const name = fileName(path);
  if (!name.toLowerCase().endsWith(".pdf")) {
    toast(t("error.notPdf"), "error");
    return null;
  }
  let password = null;
  for (;;) {
    try {
      return await invoke("open_document", { path, password });
    } catch (error) {
      if (error?.kind === "password_required" || error?.kind === "wrong_password") {
        password = await askPassword(name, error.kind === "wrong_password");
        if (password === null) return null;
        continue;
      }
      const message = error?.message ?? String(error);
      toast(t("error.open", { name, message }), "error");
      return null;
    }
  }
}

/** Dialogue « Ouvrir » ; null si annulé. */
export async function pickPdf() {
  try {
    return await invoke("pick_pdf");
  } catch (error) {
    toast(t("error.unexpected", { message: error?.message ?? String(error) }), "error");
    return null;
  }
}
