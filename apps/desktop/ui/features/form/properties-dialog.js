// Propriétés du document (E-11) : titre, auteur, sujet, mots-clés, dans un
// dialogue ; « Appliquer » est une opération annulable, écrite dans le
// fichier à l'enregistrement.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";

const FIELDS = ["title", "author", "subject", "keywords"];

/**
 * Relie le menu « Propriétés du document… » ; `onTitle(titre)` après un
 * changement (titre de la fenêtre).
 * @param {() => object|null} doc document affiché
 * @param {import("../edit/edit-session.js").EditSession} session
 */
export function attachProperties(doc, session, onTitle) {
  const dialog = byId("props-dialog");
  byId("menu-properties").addEventListener("click", async () => {
    byId("main-menu").hidden = true;
    const current = doc();
    if (!current) return;
    try {
      const metadata = await invoke("document_metadata", { id: current.id });
      for (const name of FIELDS) byId(`meta-${name}`).value = metadata?.[name] ?? "";
      dialog.showModal();
      byId("meta-title").focus();
    } catch (error) {
      toast(t("form.failed", { message: error?.message ?? String(error) }), "error");
    }
  });
  byId("props-cancel").addEventListener("click", () => dialog.close());
  byId("props-form").addEventListener("submit", async (event) => {
    event.preventDefault();
    const current = doc();
    if (!current) return dialog.close();
    const metadata = Object.fromEntries(FIELDS.map((name) => [name, byId(`meta-${name}`).value.trim()]));
    try {
      await session.apply(await invoke("set_metadata", { id: current.id, metadata }));
      dialog.close();
      onTitle(metadata.title);
    } catch (error) {
      toast(t("form.failed", { message: error?.message ?? String(error) }), "error");
    }
  });
}
