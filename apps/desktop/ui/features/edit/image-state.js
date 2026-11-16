// Calculs des images ajoutées, sans DOM : cadre initial, poignées d'angle
// (proportions gardées ou non), rotation, taille saisie. Module pur, testé
// par image-state.test.js. Coordonnées : points de la page affichée,
// origine en haut à gauche (comme edit-state.js).

/** Points par pixel d'une image à 96 ppp (taille « naturelle » à l'écran). */
const PT_PER_PX = 0.75;
/** Part de la page qu'une nouvelle image occupe au plus. */
const MAX_PAGE_SHARE = 0.6;
/** Plus petit côté à la souris (pde-core refuse en dessous de 4 pt). */
export const MIN_IMAGE_SIDE = 8;
export const IMAGE_EXTENSIONS = ["png", "jpg", "jpeg", "webp"];

function clamp(value, min, max) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

/** Fichier d'image accepté, d'après son extension. */
export function isImagePath(path) {
  const extension = path.split(".").pop()?.toLowerCase() ?? "";
  return IMAGE_EXTENSIONS.includes(extension);
}

/**
 * Cadre d'une image importée (`imported` : { image_id, width, height } en
 * pixels), centré sur `center`, réduit pour tenir dans la page.
 */
export function newImageBox(pageIndex, imported, page, center) {
  let width = imported.width * PT_PER_PX;
  let height = imported.height * PT_PER_PX;
  const fit = Math.min(1, (page.width * MAX_PAGE_SHARE) / width, (page.height * MAX_PAGE_SHARE) / height);
  width = Math.max(width * fit, MIN_IMAGE_SIDE);
  height = Math.max(height * fit, MIN_IMAGE_SIDE);
  return {
    kind: "image",
    page: pageIndex,
    top_left: {
      x: clamp(center.x - width / 2, 0, page.width - width),
      y: clamp(center.y - height / 2, 0, page.height - height),
    },
    width,
    height,
    image: imported.image_id,
    rotation: 0,
    opacity: 1,
    keep_ratio: true,
  };
}

/**
 * Coin tiré de (dx, dy) points : "nw", "ne", "sw" ou "se". Le coin opposé
 * reste en place ; avec `keepRatio`, la largeur décide et la hauteur suit.
 */
export function resizedFrame(element, corner, dx, dy, page, keepRatio = element.keep_ratio) {
  const left = element.top_left.x;
  const top = element.top_left.y;
  const right = left + element.width;
  const bottom = top + element.height;
  const west = corner.includes("w");
  const north = corner.includes("n");
  // Bords mobiles, bornés à la page.
  let width = clamp(west ? right - (left + dx) : right + dx - left, MIN_IMAGE_SIDE, west ? right : page.width - left);
  let height = clamp(north ? bottom - (top + dy) : bottom + dy - top, MIN_IMAGE_SIDE, north ? bottom : page.height - top);
  if (keepRatio) {
    const ratio = element.width / element.height;
    const maxWidth = west ? right : page.width - left;
    const maxHeight = north ? bottom : page.height - top;
    width = clamp(width, Math.max(MIN_IMAGE_SIDE, MIN_IMAGE_SIDE * ratio), Math.min(maxWidth, maxHeight * ratio));
    height = width / ratio;
  }
  return {
    ...element,
    top_left: { x: west ? right - width : left, y: north ? bottom - height : top },
    width,
    height,
  };
}

/** Quart de tour (+1 : sens horaire), autour du centre du cadre. */
export function rotated(element, direction, page) {
  const cx = element.top_left.x + element.width / 2;
  const cy = element.top_left.y + element.height / 2;
  const width = element.height;
  const height = element.width;
  return {
    ...element,
    rotation: (element.rotation + (direction > 0 ? 1 : 3)) % 4,
    width,
    height,
    top_left: {
      x: clamp(cx - width / 2, 0, page.width - width),
      y: clamp(cy - height / 2, 0, page.height - height),
    },
  };
}

/** Largeur ou hauteur saisie (points) ; l'autre suit si les proportions sont gardées. */
export function withSize(element, field, value, page) {
  const ratio = element.width / element.height;
  let width = field === "width" ? value : element.keep_ratio ? value * ratio : element.width;
  let height = field === "height" ? value : element.keep_ratio ? value / ratio : element.height;
  width = clamp(width, MIN_IMAGE_SIDE, page.width - element.top_left.x);
  height = clamp(height, MIN_IMAGE_SIDE, page.height - element.top_left.y);
  if (element.keep_ratio) {
    // Bornée par la page : on garde le côté le plus contraint.
    if (width / height > ratio + 1e-9) width = height * ratio;
    else height = width / ratio;
  }
  return { ...element, width, height };
}

/** Opacité saisie en % (5 à 100). */
export function withOpacity(element, percent) {
  const value = Number(percent);
  if (!Number.isFinite(value)) return null;
  return { ...element, opacity: clamp(Math.round(value), 5, 100) / 100 };
}
