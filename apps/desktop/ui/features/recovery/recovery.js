// Récupération après plantage (F-02) : au démarrage, chaque document laissé
// par un lancement interrompu est proposé. Récupérer rouvre le fichier et
// reprend ses ajouts ; Ignorer efface la copie ; Plus tard la garde pour le
// prochain lancement.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { relativeDate } from "../../lib/dates.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";

/** Réponse du dialogue : "restore", "discard" ou "later" (Échap compris). */
function ask(item) {
  const dialog = byId("recovery-dialog");
  const when = relativeDate(item.saved_at, Date.now() / 1000);
  byId("recovery-text").textContent = t("recovery.text", { name: item.file_name, count: item.elements, when });
  return new Promise((resolve) => {
    const buttons = [
      ["recovery-restore", "restore"],
      ["recovery-discard", "discard"],
      ["recovery-later", "later"],
    ];
    const handlers = [];
    const settle = (answer) => {
      for (const [id, handler] of handlers) byId(id).removeEventListener("click", handler);
      dialog.removeEventListener("cancel", onCancel);
      if (dialog.open) dialog.close();
      resolve(answer);
    };
    const onCancel = (event) => {
      event.preventDefault();
      settle("later");
    };
    for (const [id, answer] of buttons) {
      const handler = () => settle(answer);
      handlers.push([id, handler]);
      byId(id).addEventListener("click", handler);
    }
    dialog.addEventListener("cancel", onCancel);
    dialog.showModal();
    byId("recovery-restore").focus();
  });
}

/**
 * Propose les récupérations ; `open(chemin)` ouvre un document (ou null),
 * `restored(vue)` affiche les éléments repris. Une seule reprise par
 * lancement (un document à la fois) : `true` si elle a eu lieu.
 */
export async function offerRecoveries({ open, restored }) {
  // Erreur ou réponse vide : le démarrage continue sans rien proposer.
  const list = (await invoke("recoveries").catch(() => null)) ?? [];
  for (const item of list) {
    const answer = await ask(item);
    if (answer === "later") continue;
    if (answer === "discard") {
      await invoke("discard_recovery", { recoveryId: item.id }).catch(() => {});
      continue;
    }
    // Fichier introuvable : message déjà affiché, la copie reste pour plus tard.
    const doc = await open(item.path);
    if (!doc) return false;
    try {
      restored(await invoke("restore_recovery", { id: doc.id, recoveryId: item.id }));
      toast(t("recovery.done", { name: item.file_name }));
      return true;
    } catch (error) {
      toast(t("recovery.failed", { message: error?.message ?? String(error) }), "error");
      return false;
    }
  }
  return false;
}
