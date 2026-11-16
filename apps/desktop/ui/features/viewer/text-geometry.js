// Placement du texte et des surlignages sur une page. Module pur, testé
// par text-geometry.test.js. Les boîtes viennent de pde-app (text_view.rs) :
// points de la page affichée, origine en haut à gauche.

/** Position d'une boîte en pourcentages de la page : indépendante du zoom. */
export function percentBox(box, page) {
  return {
    left: `${(box.x / page.width) * 100}%`,
    top: `${(box.y / page.height) * 100}%`,
    width: `${(box.width / page.width) * 100}%`,
    height: `${(box.height / page.height) * 100}%`,
  };
}

/**
 * Étirement horizontal d'un segment : le texte, écrit dans la police de
 * l'interface, doit couvrir exactement la largeur qu'il a dans le PDF.
 * `measured` : largeur du texte à un corps égal à la hauteur de la boîte.
 */
export function stretch(box, measured) {
  if (!(measured > 0) || !(box.width > 0)) return 1;
  return box.width / measured;
}

/**
 * Défilement qui place `rect` (boîte d'une page) au tiers haut de la zone
 * visible. `pageBox` : position de la page sur la table (px CSS).
 */
export function scrollTopForRect(pageBox, page, rect, viewportHeight) {
  const y = pageBox.top + (rect.y / page.height) * pageBox.height;
  return Math.max(0, y - viewportHeight / 3);
}
