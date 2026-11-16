// Navigation dans les résultats de recherche. Module pur, testé.

import { t } from "../../lib/i18n.js";

/** Occurrence suivante (ou précédente si `step` = -1), en bouclant. */
export function step(index, count, direction) {
  if (count === 0) return -1;
  return (index + direction + count) % count;
}

/** Première occurrence à partir de la page affichée (recherche « vers l'avant »). */
export function firstFrom(hits, page) {
  if (hits.length === 0) return -1;
  const index = hits.findIndex((hit) => hit.page >= page);
  return index === -1 ? 0 : index;
}

/** « 3 sur 12 », « Aucun résultat », « 1 sur 1000+ ». */
export function countLabel(index, count, truncated) {
  if (count === 0) return t("search.none");
  const total = truncated ? `${count}+` : String(count);
  return t("search.count", { index: index + 1, total });
}
