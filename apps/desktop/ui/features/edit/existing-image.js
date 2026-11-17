// Images d'origine du document (E-06), sans DOM. Une image d'origine est
// proposée à la sélection comme un élément « synthétique » (identifiant
// "orig:page:rang") tant qu'elle n'est pas modifiée ; la première
// modification crée l'élément `existing_image` (nouveau cadre, ou aucun si
// elle est supprimée). Ses calculs de place reprennent ceux des images
// ajoutées, sur son cadre. Coordonnées : points, origine en haut à gauche.

export const EXISTING = "existing_image";

export const isExisting = (element) => element?.kind === EXISTING;

export const isSynthetic = (id) => typeof id === "string" && id.startsWith("orig:");

const area = (box) => ({ top_left: { x: box.x, y: box.y }, width: box.width, height: box.height });

/** Image d'origine lue par Rust ({ object, bounds }) → élément proposé à la sélection. */
export function syntheticItem(pageIndex, image) {
  const original = area(image.bounds);
  return {
    id: `orig:${pageIndex}:${image.object}`,
    synthetic: true,
    bounds: { ...image.bounds },
    element: { kind: EXISTING, page: pageIndex, object: image.object, original, frame: original },
  };
}

/** Images d'origine d'une page encore à proposer : celles qu'aucun élément ne modifie. */
export function syntheticItems(pageIndex, images, items) {
  const edited = new Set(items.filter((i) => isExisting(i.element) && i.element.page === pageIndex).map((i) => i.element.object));
  return images.filter((image) => !edited.has(image.object)).map((image) => syntheticItem(pageIndex, image));
}

/** Le cadre vu comme une image ajoutée (déplacer, redimensionner, saisir en mm). */
export function asImage(element) {
  const frame = element.frame ?? element.original;
  return {
    kind: "image",
    page: element.page,
    top_left: { ...frame.top_left },
    width: frame.width,
    height: frame.height,
    image: 0,
    rotation: 0,
    opacity: 1,
    keep_ratio: true,
  };
}

/** Le cadre d'une image ajoutée reporté sur l'image d'origine. */
export function withFrame(element, image) {
  return { ...element, frame: { top_left: { ...image.top_left }, width: image.width, height: image.height } };
}

/** Image d'origine supprimée (annulable : l'élément garde son cadre d'origine). */
export function withoutImage(element) {
  return { ...element, frame: null };
}

/** Nouvelle image dans le cadre d'une image remplacée : centrée, à ses proportions. */
export function fittedImageBox(element, imported) {
  const frame = element.frame ?? element.original;
  const scale = Math.min(frame.width / imported.width, frame.height / imported.height);
  const width = imported.width * scale;
  const height = imported.height * scale;
  return {
    kind: "image",
    page: element.page,
    top_left: { x: frame.top_left.x + (frame.width - width) / 2, y: frame.top_left.y + (frame.height - height) / 2 },
    width,
    height,
    image: imported.image_id,
    rotation: 0,
    opacity: 1,
    keep_ratio: true,
  };
}
