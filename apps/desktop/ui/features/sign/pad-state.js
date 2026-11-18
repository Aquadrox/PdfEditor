// Calculs de la signature manuscrite (module pur, testé par
// pad-state.test.js) : détourage de l'encre, fond clair rendu transparent
// pour une image importée, ajustement d'une image dans un cadre.

/** Marge laissée autour de l'encre détourée, en pixels. */
export const MARGIN = 8;

/**
 * Boîte de l'encre (pixels non transparents) d'une image RGBA, marge
 * comprise et bornée à l'image ; null si l'image est vide.
 */
export function inkBounds(data, width, height, margin = MARGIN) {
  let [x0, y0, x1, y1] = [width, height, -1, -1];
  for (let y = 0; y < height; y++) {
    for (let x = 0; x < width; x++) {
      if (data[(y * width + x) * 4 + 3] > 8) {
        if (x < x0) x0 = x;
        if (x > x1) x1 = x;
        if (y < y0) y0 = y;
        if (y > y1) y1 = y;
      }
    }
  }
  if (x1 < 0) return null;
  const left = Math.max(0, x0 - margin);
  const top = Math.max(0, y0 - margin);
  return { x: left, y: top, width: Math.min(width, x1 + margin + 1) - left, height: Math.min(height, y1 + margin + 1) - top };
}

/**
 * Photo ou scan d'une signature : le papier clair devient transparent, en
 * fondu pour garder des bords doux. Modifie `data` (RGBA) en place.
 */
export function clearPaper(data, threshold = 200) {
  for (let i = 0; i < data.length; i += 4) {
    const light = 0.299 * data[i] + 0.587 * data[i + 1] + 0.114 * data[i + 2];
    if (light >= 245) data[i + 3] = 0;
    else if (light > threshold) data[i + 3] = Math.round((data[i + 3] * (245 - light)) / (245 - threshold));
  }
  return data;
}

/** Image `w` × `h` ajustée dans un cadre, proportions gardées, centrée. */
export function contain(w, h, frameWidth, frameHeight) {
  const scale = Math.min(frameWidth / w, frameHeight / h);
  const width = w * scale;
  const height = h * scale;
  return { x: (frameWidth - width) / 2, y: (frameHeight - height) / 2, width, height };
}

/** Corps de police (px) pour que `measure(corps)` tienne dans `width`, borné. */
export function fittingSize(measure, width, max = 120, min = 24) {
  let size = max;
  while (size > min && measure(size) > width) size -= 4;
  return size;
}
