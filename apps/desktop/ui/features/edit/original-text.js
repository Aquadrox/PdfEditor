// Texte d'origine du document (E-05), sans DOM. Avec l'outil « Texte du
// document », chaque ligne de texte de la page (bloc lu par Rust) se
// propose à la modification ; la première modification crée l'élément
// `original_text` (nouveau texte, vide s'il est supprimé). Coordonnées :
// points, origine en haut à gauche.

export const ORIGINAL_TEXT = "original_text";

export const isOriginalText = (element) => element?.kind === ORIGINAL_TEXT;

/** Le bloc a été vidé : le texte d'origine est supprimé. */
export const isRemovedText = (element) => isOriginalText(element) && element.text.trim() === "";

const area = (box) => ({ top_left: { x: box.x, y: box.y }, width: box.width, height: box.height });

/** Bloc lu par Rust ({ objects, bounds, text, font_name, style }) → élément non modifié. */
export function blockElement(pageIndex, block) {
  return {
    kind: ORIGINAL_TEXT,
    page: pageIndex,
    objects: [...block.objects],
    original: area(block.bounds),
    before: block.text,
    text: block.text,
    style: { ...block.style },
  };
}

/** Blocs d'une page encore à proposer : ceux qu'aucun élément ne modifie. */
export function blockItems(pageIndex, blocks, items) {
  const edited = new Set(items.filter((i) => isOriginalText(i.element) && i.element.page === pageIndex).map((i) => i.element.objects[0]));
  return blocks
    .filter((block) => !edited.has(block.objects[0]))
    .map((block) => ({
      id: `otext:${pageIndex}:${block.objects[0]}`,
      bounds: { ...block.bounds },
      fontName: block.font_name,
      element: blockElement(pageIndex, block),
    }));
}

/** Le texte saisi, sur une ligne (le texte d'origine se modifie ligne par ligne). */
export function oneLine(text) {
  return text.replace(/\s*[\r\n]+\s*/g, " ");
}

export function withNewText(element, text) {
  return { ...element, text: oneLine(text) };
}

/**
 * Zone de saisie posée sur le bloc : même place et même corps, assez large
 * pour un texte plus long que l'original.
 */
export function editorBox(element, page) {
  const { top_left, width } = element.original;
  return {
    kind: "text",
    page: element.page,
    top_left: { ...top_left },
    width: Math.max(width, Math.min(page.width - top_left.x, width + element.style.size * 6)),
    text: element.text,
    style: { ...element.style, align: "left", line_height: 1.2 },
  };
}
