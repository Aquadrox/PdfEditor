// Zoom de la visionneuse : paliers, ajustements, affichage. Module pur, testé
// par zoom.test.js.

/** 100 % = taille réelle : 1 point (1/72 pouce) = 96/72 pixels CSS. */
export const CSS_PX_PER_POINT = 96 / 72;
export const MIN_ZOOM = 0.1;
export const MAX_ZOOM = 8;
/** Paliers des boutons + et − (et de Ctrl + / Ctrl −). */
export const ZOOM_STEPS = [
  0.25, 0.33, 0.5, 0.67, 0.75, 0.9, 1, 1.1, 1.25, 1.5, 1.75, 2, 2.5, 3, 4, 5, 6, 8,
];
/** Marge laissée de chaque côté de la page par les ajustements (px CSS). */
export const FIT_MARGIN = 24;

const EPSILON = 1e-6;

export function clampZoom(zoom) {
  if (!Number.isFinite(zoom)) return 1;
  return Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, zoom));
}

/** Palier suivant, au-dessus du zoom actuel (même s'il est entre deux paliers). */
export function zoomIn(zoom) {
  return ZOOM_STEPS.find((step) => step > zoom + EPSILON) ?? MAX_ZOOM;
}

/** Palier précédent, en dessous du zoom actuel. */
export function zoomOut(zoom) {
  return [...ZOOM_STEPS].reverse().find((step) => step < zoom - EPSILON) ?? ZOOM_STEPS[0];
}

/** Zoom pour que la page la plus large tienne dans `available` px CSS. */
export function fitWidth(pages, available) {
  const widest = Math.max(...pages.map((p) => p.width), 1);
  return clampZoom((available - 2 * FIT_MARGIN) / (widest * CSS_PX_PER_POINT));
}

/** Zoom pour qu'une page entière tienne dans la zone visible. */
export function fitPage(page, availableWidth, availableHeight) {
  const byWidth = (availableWidth - 2 * FIT_MARGIN) / (page.width * CSS_PX_PER_POINT);
  const byHeight = (availableHeight - 2 * FIT_MARGIN) / (page.height * CSS_PX_PER_POINT);
  return clampZoom(Math.min(byWidth, byHeight));
}

/** « 110 % », avec l'espace insécable du français. */
export function formatZoom(zoom) {
  return `${Math.round(zoom * 100)} %`;
}

/**
 * Ajustement à l'ouverture selon le réglage display.default_zoom :
 * "width", "page", ou null pour la taille réelle (zoom 1).
 */
export function fitModeFor(defaultZoom) {
  if (defaultZoom === "fit_page") return "page";
  if (defaultZoom === "actual") return null;
  return "width";
}
