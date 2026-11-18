// Copie d'une réponse de l'assistant IA dans le presse-papiers : le texte
// tel que reçu (Markdown léger, citations [p. n] comprises). Le texte reste
// aussi sélectionnable à la souris (styles/ai.css).

import { toast } from "../../components/toast.js";
import { t } from "../../lib/i18n.js";

/** Copie `entry.text` ; à défaut d'API presse-papiers, copie le texte affiché. */
export async function copyAnswer(entry) {
  try {
    await navigator.clipboard.writeText(entry.text);
  } catch {
    const range = document.createRange();
    range.selectNodeContents(entry.body);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    const copied = document.execCommand("copy");
    selection.removeAllRanges();
    if (!copied) return toast(t("ai.copyFailed"), "error");
  }
  toast(t("ai.copied"));
}
