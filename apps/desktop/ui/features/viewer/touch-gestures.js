// Gestes au doigt sur la table (A-01) : pincer pour zoomer (deux doigts,
// qui font aussi glisser la page), double tapotement pour grossir ou revenir
// à la largeur de l'écran. Pendant le pincement, seule une transformation
// CSS agrandit les pages (fluide) ; au relâchement, le vrai zoom est appliqué
// et les images nettes sont redemandées.
//
// Événements « touch » plutôt que « pointer » : seul un `touchmove` annulé
// empêche le navigateur de faire défiler pendant que deux doigts pincent.

import { distance, doubleTapTarget, isDoubleTap, midpoint, pinchZoom, TAP_SLOP } from "./touch-zoom.js";

/** Cibles qui gardent leur propre double tapotement (saisie, éléments, champs). */
const OWN_GESTURES =
  "input, textarea, select, button, a, [contenteditable], .form-field, .edit-layer.active, .design-layer, .sign-layer.placing, .sig-box";

/**
 * @param {import("./viewer.js").Viewer} viewer
 * @param {() => boolean} doubleTapAllowed le mode affiché laisse le double tapotement à la page
 */
export function attachTouchGestures(viewer, doubleTapAllowed) {
  const desk = viewer.desk;
  const container = viewer.container;
  let pinch = null;
  let tap = null;
  let lastTap = null;

  /** Point d'un doigt dans la table (px, coin haut gauche de la zone visible). */
  const local = (touch) => {
    const rect = desk.getBoundingClientRect();
    return { x: touch.clientX - rect.left, y: touch.clientY - rect.top };
  };

  function startPinch(event) {
    const [a, b] = [local(event.touches[0]), local(event.touches[1])];
    const from = midpoint(a, b);
    pinch = { from, to: from, startDistance: distance(a, b), startZoom: viewer.zoom, zoom: viewer.zoom };
    // Point du contenu sous les doigts : origine de l'agrandissement.
    const originX = desk.scrollLeft + from.x - container.offsetLeft;
    const originY = desk.scrollTop + from.y - container.offsetTop;
    container.style.transformOrigin = `${originX}px ${originY}px`;
    container.style.willChange = "transform";
    tap = null;
  }

  function movePinch(event) {
    const [a, b] = [local(event.touches[0]), local(event.touches[1])];
    pinch.to = midpoint(a, b);
    pinch.zoom = pinchZoom(pinch.startZoom, pinch.startDistance, distance(a, b));
    const scale = pinch.zoom / pinch.startZoom;
    const dx = pinch.to.x - pinch.from.x;
    const dy = pinch.to.y - pinch.from.y;
    container.style.transform = `translate(${dx}px, ${dy}px) scale(${scale})`;
  }

  function endPinch() {
    const { zoom, from, to, startZoom } = pinch;
    pinch = null;
    container.style.removeProperty("transform");
    container.style.removeProperty("transform-origin");
    container.style.removeProperty("will-change");
    if (Math.abs(zoom - startZoom) > 0.001 || from.x !== to.x || from.y !== to.y) viewer.zoomAt(zoom, from, to);
  }

  desk.addEventListener(
    "touchstart",
    (event) => {
      if (!viewer.doc) return;
      if (event.touches.length === 2) {
        startPinch(event);
        return;
      }
      if (event.touches.length > 2 && pinch) {
        endPinch();
        return;
      }
      const touch = event.touches[0];
      tap = event.touches.length === 1 ? { ...local(touch), time: event.timeStamp, target: event.target } : null;
    },
    { passive: true },
  );

  desk.addEventListener(
    "touchmove",
    (event) => {
      if (pinch && event.touches.length >= 2) {
        if (event.cancelable) event.preventDefault();
        movePinch(event);
        return;
      }
      if (tap && event.touches.length === 1) {
        const point = local(event.touches[0]);
        if (Math.hypot(point.x - tap.x, point.y - tap.y) > TAP_SLOP) tap = null;
      }
    },
    { passive: false },
  );

  desk.addEventListener(
    "touchend",
    (event) => {
      if (pinch) {
        if (event.touches.length < 2) endPinch();
        lastTap = null;
        return;
      }
      if (!tap || event.touches.length > 0) return;
      const current = { x: tap.x, y: tap.y, time: event.timeStamp };
      const target = tap.target;
      tap = null;
      if (!doubleTapAllowed() || target.closest?.(OWN_GESTURES)) {
        lastTap = null;
        return;
      }
      if (!isDoubleTap(lastTap, current)) {
        lastTap = current;
        return;
      }
      lastTap = null;
      // Pas de clic ni de sélection de mot pour ce second tapotement.
      if (event.cancelable) event.preventDefault();
      const next = doubleTapTarget(viewer.zoom, viewer.fitMode, viewer.widthZoom());
      viewer.zoomAt(next.zoom, current, current, next.fit);
    },
    { passive: false },
  );

  desk.addEventListener("touchcancel", () => {
    if (pinch) endPinch();
    tap = null;
  });
}
