// Enregistrer, Enregistrer sous, la confirmation avant d'écrire des zones
// caviardées (E-07), et la question posée avant de perdre des
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
  if (!(await confirmRedaction(session))) return null;
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
  if (!(await confirmRedaction(session))) return null;
  try {
    // Dossier et nom de copie proposés par Rust (réglage Fichiers).
    const saved = await invoke("save_document_as", { id: session.doc.id });
    if (!saved) return null;
    await session.reload();
    toast(t("save.done", { name: saved.file_name }));
    return saved;
  } catch (error) {
    toast(t("save.failed", { message: message(error) }), "error");
    return null;
  }
}

/**
 * Zones caviardées pas encore confirmées : le contenu dessous va quitter le
 * fichier, on le dit avant d'écrire. `true` pour continuer.
 */
async function confirmRedaction(session) {
  const ids = session.view.elements.filter((e) => e.element.kind === "redact").map((e) => e.id);
  if (ids.every((id) => session.confirmedRedactions.has(id))) return true;
  const dialog = byId("redact-dialog");
  byId("redact-text").textContent = t("redact.text", { name: session.doc.file_name });
  const confirmed = await new Promise((resolve) => {
    const answer = (value) => () => {
      dialog.removeEventListener("cancel", cancel);
      byId("redact-confirm").removeEventListener("click", confirm);
      byId("redact-cancel").removeEventListener("click", cancel);
      if (dialog.open) dialog.close();
      resolve(value);
    };
    const confirm = answer(true);
    const cancel = answer(false);
    byId("redact-confirm").addEventListener("click", confirm);
    byId("redact-cancel").addEventListener("click", cancel);
    // Échap : comme « Annuler ».
    dialog.addEventListener("cancel", cancel);
    dialog.showModal();
    byId("redact-cancel").focus();
  });
  if (confirmed) ids.forEach((id) => session.confirmedRedactions.add(id));
  return confirmed;
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
