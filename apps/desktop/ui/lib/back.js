// Bouton Retour d'Android : il remonte l'historique du WebView. Un panneau
// ouvert par-dessus la page (tiroir, menu, Réglages) y pose une étape ; le
// Retour la consomme et ferme le panneau au lieu de quitter l'application.
// Sur PC, l'historique n'est jamais parcouru : sans effet visible.

const stack = [];
/** Retours lancés par release() : leur `popstate` ne ferme rien. */
let skipped = 0;

window.addEventListener("popstate", () => {
  if (skipped > 0) {
    skipped -= 1;
    return;
  }
  stack.pop()?.();
});

/**
 * `close` sera appelé au prochain Retour. Renvoie `release`, à appeler quand
 * le panneau se ferme autrement (bouton, voile) : l'étape est retirée.
 */
export function onBack(close) {
  const entry = () => close();
  stack.push(entry);
  history.pushState({ pdeBack: stack.length }, "");
  return () => {
    const index = stack.lastIndexOf(entry);
    if (index < 0) return;
    stack.splice(index, 1);
    skipped += 1;
    history.back();
  };
}
