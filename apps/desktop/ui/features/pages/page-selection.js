// Mode Pages : sélection des pages (clic, Ctrl, Maj, flèches) et place de
// dépôt d'un glisser. Fonctions pures, sans DOM (tests : node --test).
//
// Une sélection : { pages: numéros triés, anchor: départ des Maj+clic,
// focus: dernière page désignée (flèches) }.

const range = (from, to) => Array.from({ length: Math.abs(to - from) + 1 }, (_, i) => Math.min(from, to) + i);
const sorted = (pages) => [...new Set(pages)].sort((a, b) => a - b);

export function selectOnly(index) {
  return { pages: [index], anchor: index, focus: index };
}

export function selectAll(count) {
  if (count <= 0) return { pages: [], anchor: null, focus: null };
  return { pages: range(0, count - 1), anchor: 0, focus: count - 1 };
}

/** Pages données (après une opération : pages copiées, déplacées…). */
export function selectPages(pages) {
  const list = sorted(pages);
  return { pages: list, anchor: list[0] ?? null, focus: list.at(-1) ?? null };
}

/** Clic sur une page : seule, Ctrl (ajoute ou retire), Maj (plage depuis l'ancre). */
export function clicked(selection, index, { shift = false, toggle = false } = {}) {
  if (shift && selection.anchor !== null) {
    const span = range(selection.anchor, index);
    const pages = toggle ? sorted([...selection.pages, ...span]) : span;
    return { pages, anchor: selection.anchor, focus: index };
  }
  if (toggle) {
    const pages = selection.pages.includes(index)
      ? selection.pages.filter((p) => p !== index)
      : sorted([...selection.pages, index]);
    return { pages, anchor: index, focus: index };
  }
  return selectOnly(index);
}

/** Flèches, Début, Fin dans une grille de `columns` colonnes ; Maj étend. */
export function moved(selection, key, count, columns, shift = false) {
  if (count === 0) return selection;
  const from = selection.focus ?? 0;
  const steps = { ArrowLeft: -1, ArrowRight: 1, ArrowUp: -columns, ArrowDown: columns };
  let to;
  if (key === "Home") to = 0;
  else if (key === "End") to = count - 1;
  else if (key in steps) to = Math.min(count - 1, Math.max(0, from + steps[key]));
  else return selection;
  if (!shift) return selectOnly(to);
  const anchor = selection.anchor ?? from;
  return { pages: range(anchor, to), anchor, focus: to };
}

/** Après un changement du nombre de pages : pages disparues retirées, une au moins choisie. */
export function clamped(selection, count) {
  const pages = selection.pages.filter((p) => p < count);
  if (pages.length > 0 || count === 0) {
    const inside = (p) => (p !== null && p < count ? p : (pages[0] ?? null));
    return { pages, anchor: inside(selection.anchor), focus: inside(selection.focus) };
  }
  return selectOnly(Math.min(selection.focus ?? 0, count - 1));
}

/**
 * Place de dépôt : devant quelle page insérer (0 à nombre de pages), d'après
 * le pointeur et le cadre de chaque carte, dans l'ordre des pages
 * ({ left, top, width, height }, mêmes coordonnées que le pointeur).
 */
export function dropIndex(boxes, x, y) {
  if (boxes.length === 0) return 0;
  // Rangée sous le pointeur, sinon la plus proche verticalement.
  const middle = (b) => b.top + b.height / 2;
  let rowTop = boxes.find((b) => y >= b.top && y < b.top + b.height)?.top;
  if (rowTop === undefined) {
    rowTop = boxes.reduce((best, b) => (Math.abs(middle(b) - y) < Math.abs(middle(best) - y) ? b : best)).top;
  }
  const row = boxes.map((box, index) => ({ box, index })).filter(({ box }) => box.top === rowTop);
  const before = row.find(({ box }) => x < box.left + box.width / 2);
  return before ? before.index : row.at(-1).index + 1;
}

/** Déposer les pages à `to` les laisse où elles sont (bloc contigu déposé sur lui-même). */
export function isStill(pages, to) {
  const list = sorted(pages);
  const contiguous = list.every((p, i) => i === 0 || p === list[i - 1] + 1);
  return contiguous && to >= list[0] && to <= list.at(-1) + 1;
}

/** Où insérer une page blanche : après la dernière page choisie, sinon à la fin. */
export function insertionPoint(selection, count) {
  return selection.pages.length > 0 ? selection.pages.at(-1) + 1 : count;
}
