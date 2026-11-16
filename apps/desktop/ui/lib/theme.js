// Thème de l'interface : celui du système, ou forcé clair / sombre
// (styles/tokens.css lit l'attribut data-theme).

export function applyTheme(theme) {
  if (theme === "light" || theme === "dark") document.documentElement.dataset.theme = theme;
  else delete document.documentElement.dataset.theme;
}
