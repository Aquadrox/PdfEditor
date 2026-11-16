// Raccourcis clavier : touche appuyée → notation de pde-core (« Ctrl+Shift+F »),
// et affichage en français (« Ctrl+Maj+F »). Module pur, testé.

const NAMED = {
  "+": "Plus",
  "=": "Plus",
  "-": "Minus",
  _: "Minus",
  ",": "Comma",
  ".": "Period",
  " ": "Space",
  Enter: "Enter",
  Tab: "Tab",
  Home: "Home",
  End: "End",
  PageUp: "PageUp",
  PageDown: "PageDown",
  Delete: "Delete",
};

const MODIFIER_KEYS = new Set(["Control", "Shift", "Alt", "Meta", "AltGraph"]);

/** Touche principale, ou null (modificateur seul, touche inconnue). */
function mainKey(event) {
  if (MODIFIER_KEYS.has(event.key)) return null;
  // Chiffres : touche physique, car sur un clavier AZERTY « 0 » demande Maj.
  const digit = /^(?:Digit|Numpad)(\d)$/.exec(event.code ?? "");
  if (digit) return digit[1];
  if (/^[a-z0-9]$/i.test(event.key)) return event.key.toUpperCase();
  if (/^F([1-9]|1[0-2])$/.test(event.key)) return event.key;
  return NAMED[event.key] ?? null;
}

/** Notation du raccourci de `event`, ou null s'il n'en forme pas un. */
export function eventToShortcut(event) {
  const key = mainKey(event);
  if (!key) return null;
  const parts = [];
  if (event.ctrlKey || event.metaKey) parts.push("Ctrl");
  if (event.altKey) parts.push("Alt");
  // + et − demandent souvent Maj : Ctrl+Maj+= reste Ctrl+Plus.
  if (event.shiftKey && key !== "Plus" && key !== "Minus") parts.push("Shift");
  parts.push(key);
  return parts.join("+");
}

/** Même règle que pde-core : Ctrl ou Alt obligatoire, sauf touches F1 à F12. */
export function isAcceptable(shortcut) {
  const parts = shortcut.split("+");
  const key = parts.at(-1);
  return /^F([1-9]|1[0-2])$/.test(key) || parts.includes("Ctrl") || parts.includes("Alt");
}

const LABELS = {
  Shift: "Maj",
  Plus: "+",
  Minus: "−",
  Comma: ",",
  Period: ".",
  Space: "Espace",
  Enter: "Entrée",
  Delete: "Suppr",
  Home: "Début",
  End: "Fin",
  PageUp: "Page préc.",
  PageDown: "Page suiv.",
};

/** Affichage : « Ctrl+Maj+F », « Ctrl++ ». */
export function shortcutLabel(shortcut) {
  return shortcut
    .split("+")
    .map((part) => LABELS[part] ?? part)
    .join("+");
}

/** Table raccourci → action, pour retrouver l'action d'une touche. */
export function keymap(shortcuts) {
  return new Map(Object.entries(shortcuts).map(([action, shortcut]) => [shortcut, action]));
}
