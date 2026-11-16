// Traduction de l'interface : t(clé, paramètres) et attributs data-i18n*.
// Français seulement pour l'instant ; une langue = un fichier messages-xx.js.

import fr from "./messages-fr.js";

const catalogs = { fr };
let messages = fr;

/** Choisit la langue ; inconnue → français. */
export function setLocale(locale) {
  messages = catalogs[locale] ?? fr;
}

/** Texte de `key`, `{nom}` remplacé par `params.nom` ; clé inconnue → la clé. */
export function t(key, params = {}) {
  const template = messages[key] ?? key;
  return template.replace(/\{(\w+)\}/g, (match, name) =>
    name in params ? String(params[name]) : match,
  );
}

/**
 * Traduit un fragment de page :
 * data-i18n → texte, data-i18n-label → aria-label et info-bulle (boutons),
 * data-i18n-aria → aria-label seul (zones), data-i18n-title → info-bulle seule.
 */
export function translate(root) {
  for (const el of root.querySelectorAll("[data-i18n]")) {
    el.textContent = t(el.dataset.i18n);
  }
  for (const el of root.querySelectorAll("[data-i18n-label]")) {
    const text = t(el.dataset.i18nLabel);
    el.setAttribute("aria-label", text);
    el.title = text;
  }
  for (const el of root.querySelectorAll("[data-i18n-aria]")) {
    el.setAttribute("aria-label", t(el.dataset.i18nAria));
  }
  for (const el of root.querySelectorAll("[data-i18n-title]")) {
    el.title = t(el.dataset.i18nTitle);
  }
}
