// Disposition compacte (téléphone, docs/DESIGN.md § Tactile) : fenêtre
// étroite, ou écran tactile peu haut (téléphone en paysage). La classe
// `compact` posée sur <html> porte toute la mise en page (styles/mobile.css) ;
// les modules qui changent de comportement lisent isCompact().

/** Même règle que la feuille de style : à modifier aux deux endroits. */
export const COMPACT_QUERY = "(max-width: 760px), (pointer: coarse) and (max-height: 500px)";

const query = window.matchMedia(COMPACT_QUERY);
const listeners = new Set();

function apply() {
  document.documentElement.classList.toggle("compact", query.matches);
  for (const listener of listeners) listener(query.matches);
}

query.addEventListener("change", apply);
apply();

export function isCompact() {
  return query.matches;
}

/** Écran tactile (doigt) : pas de survol, pas de glisser-déposer de fichiers. */
export function isTouch() {
  return window.matchMedia("(pointer: coarse)").matches;
}

/** `listener(compact)` à chaque passage d'une disposition à l'autre. */
export function onCompactChange(listener) {
  listeners.add(listener);
  return () => listeners.delete(listener);
}
