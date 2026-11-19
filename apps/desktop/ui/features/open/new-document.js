// Nouveau document (E-13) : format, orientation et nombre de pages dans un
// dialogue ; le PDF vierge est créé par Rust et ouvert sans titre
// (« Enregistrer » demandera où l'écrire).

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";

/** Demande de création lue dans le dialogue. */
export function readRequest(paper, orientation, pages) {
  const count = Math.min(Math.max(Number.parseInt(pages, 10) || 1, 1), 200);
  return { paper, landscape: orientation === "landscape", pages: count };
}

/**
 * Relie le dialogue ; `onCreated(document, { form })` reçoit le document
 * créé, `form` vrai si l'utilisateur veut y concevoir un formulaire.
 * @param {() => Promise<boolean>} canLeave avant de remplacer le document affiché
 */
export function attachNewDocument(canLeave, onCreated) {
  const dialog = byId("new-dialog");
  const open = async () => {
    byId("main-menu").hidden = true;
    if (!(await canLeave())) return;
    dialog.showModal();
    byId("new-paper").focus();
  };
  byId("menu-new").addEventListener("click", open);
  byId("home-new").addEventListener("click", open);
  byId("new-cancel").addEventListener("click", () => dialog.close());
  byId("new-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const request = readRequest(byId("new-paper").value, byId("new-orientation").value, byId("new-pages").value);
    try {
      const doc = await invoke("new_document", { request });
      dialog.close();
      await onCreated(doc, { form: byId("new-design").checked });
    } catch (error) {
      toast(t("newDoc.failed", { message: error?.message ?? String(error) }), "error");
    }
  });
}
