// Différences mot à mot entre une ligne et sa modification proposée
// (IA-09) : plus longue sous-suite commune sur les mots (espaces gardés).
// Module pur, testé ; l'affichage passe par textContent.

/** Mots et espaces, dans l'ordre. */
function tokens(text) {
  return text.split(/(\s+)/).filter((t) => t !== "");
}

/**
 * Morceaux { kind: "same" | "del" | "ins", text } qui mènent de `before`
 * à `after` ; morceaux voisins de même genre réunis.
 */
export function wordDiff(before, after) {
  const a = tokens(before);
  const b = tokens(after);
  // Longueurs des plus longues sous-suites communes, depuis la fin.
  const lcs = Array.from({ length: a.length + 1 }, () => new Array(b.length + 1).fill(0));
  for (let i = a.length - 1; i >= 0; i--) {
    for (let j = b.length - 1; j >= 0; j--) {
      lcs[i][j] = a[i] === b[j] ? lcs[i + 1][j + 1] + 1 : Math.max(lcs[i + 1][j], lcs[i][j + 1]);
    }
  }
  const parts = [];
  const push = (kind, text) => {
    const last = parts.at(-1);
    if (last?.kind === kind) last.text += text;
    else parts.push({ kind, text });
  };
  let i = 0;
  let j = 0;
  while (i < a.length || j < b.length) {
    if (i < a.length && j < b.length && a[i] === b[j]) {
      push("same", a[i]);
      i++;
      j++;
    } else if (i < a.length && (j === b.length || lcs[i + 1][j] >= lcs[i][j + 1])) {
      // Suppression d'abord : « ~~ancien~~ nouveau » se lit mieux.
      push("del", a[i++]);
    } else {
      push("ins", b[j++]);
    }
  }
  return parts;
}
