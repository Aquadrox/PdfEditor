// Ouverture d'un fichier : mot de passe demandé si besoin, erreurs expliquées.

import { toast } from "../../components/toast.js";
import { fileName } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { askPassword } from "./password-dialog.js";

/** Documents de bureautique importés en PDF (E-13). */
const IMPORTABLE = ["docx", "doc", "odt", "rtf"];

/** Le fichier est un document Word, OpenDocument ou RTF à importer. */
export function isImportable(path) {
  const ext = fileName(path).split(".").pop()?.toLowerCase();
  return IMPORTABLE.includes(ext);
}

/**
 * Convertit un document Word (Word, LibreOffice, ou import intégré) et
 * l'ouvre sans titre ; null en cas d'échec.
 */
export async function importPath(path) {
  const name = fileName(path);
  toast(t("import.running", { name }));
  try {
    const imported = await invoke("import_document", { path });
    if (imported.notice) toast(imported.notice);
    else toast(t("import.done", { name, converter: imported.converter }));
    return imported.document;
  } catch (error) {
    toast(t("import.failed", { name, message: error?.message ?? String(error) }), "error");
    return null;
  }
}

/** Ouvre `path` ; renvoie la description du document, ou null (annulé, erreur). */
export async function openPath(path) {
  const name = fileName(path);
  if (isImportable(path)) return importPath(path);
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

/** Dialogue « Importer un document » ; null si annulé. */
export async function pickDocument() {
  try {
    return await invoke("pick_document_to_import");
  } catch (error) {
    toast(t("import.failed", { name: "", message: error?.message ?? String(error) }), "error");
    return null;
  }
}
