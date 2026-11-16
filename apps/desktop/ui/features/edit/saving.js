// Enregistrer, Enregistrer sous, et la question posée avant de perdre des
// modifications (fermer le document ou la fenêtre, ouvrir un autre fichier).

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";

const message = (error) => error?.message ?? String(error);

/**
 * Enregistre à la place du fichier ouvert. `true` si c'est fait.
 * @param {import("./edit-session.js").EditSession} session
 */
export async function saveDocument(session) {
  try {
    const saved = await invoke("save_document", { id: session.doc.id });
    await session.reload();
    toast(t("save.done", { name: saved.file_name }));
    return saved;
  } catch (error) {
    toast(t("save.failed", { message: message(error) }), "error");
    return null;
  }
}

/** Demande le fichier puis enregistre ; `null` si annulé ou en échec. */
export async function saveDocumentAs(session) {
  try {
    const saved = await invoke("save_document_as", { id: session.doc.id, suggestedName: session.doc.file_name });
    if (!saved) return null;
    await session.reload();
    toast(t("save.done", { name: saved.file_name }));
    return saved;
  } catch (error) {
    toast(t("save.failed", { message: message(error) }), "error");
    return null;
  }
}

let pending = null;
let wired = false;

/** Ferme le dialogue et donne la réponse à la question en cours. */
function settle(answer) {
  const resolve = pending;
  pending = null;
  const dialog = byId("unsaved-dialog");
  if (dialog.open) dialog.close();
  resolve?.(answer);
}

/**
 * Réponse du dialogue : "save", "discard" ou "cancel". Les boutons sont
 * reliés une seule fois et répondent au clic : l'événement `close`, qui
 * arrive plus tard, pourrait sinon répondre à la question suivante.
 */
function ask(name) {
  const dialog = byId("unsaved-dialog");
  if (!wired) {
    wired = true;
    byId("unsaved-save").addEventListener("click", () => settle("save"));
    byId("unsaved-discard").addEventListener("click", () => settle("discard"));
    byId("unsaved-cancel").addEventListener("click", () => settle("cancel"));
    // Échap : même effet qu'« Annuler ».
    dialog.addEventListener("cancel", () => settle("cancel"));
  }
  settle("cancel");
  byId("unsaved-text").textContent = t("unsaved.text", { name });
  return new Promise((resolve) => {
    pending = resolve;
    dialog.showModal();
    byId("unsaved-save").focus();
  });
}

/**
 * Avant de quitter le document : `true` si on peut continuer (rien à
 * enregistrer, enregistré, ou modifications abandonnées).
 * @param {() => Promise<object|null>} save
 */
export async function confirmDiscard(session, save) {
  if (!session.dirty) return true;
  const answer = await ask(session.doc.file_name);
  if (answer === "discard") return true;
  if (answer === "save") return Boolean(await save());
  return false;
}
