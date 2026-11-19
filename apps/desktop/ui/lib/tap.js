// Au doigt, un appui sur la page peut être le début d'un défilement : les
// actions qui posent quelque chose (texte, note) attendent le tapotement,
// c'est-à-dire un relâchement presque sur place. À la souris ou au stylet,
// elles partent tout de suite, comme avant.

import { TAP_SLOP } from "../features/viewer/touch-zoom.js";

/** Appelle `act()` maintenant (souris), ou au relâchement d'un tapotement (doigt). */
export function afterTap(event, act) {
  if (event.pointerType !== "touch") {
    act();
    return;
  }
  const start = { id: event.pointerId, x: event.clientX, y: event.clientY };
  const stop = () => {
    window.removeEventListener("pointerup", up, true);
    window.removeEventListener("pointercancel", cancel, true);
  };
  const up = (e) => {
    if (e.pointerId !== start.id) return;
    stop();
    if (Math.hypot(e.clientX - start.x, e.clientY - start.y) <= TAP_SLOP) act();
  };
  // Défilement commencé par le navigateur : ce n'était pas un tapotement.
  const cancel = (e) => {
    if (e.pointerId === start.id) stop();
  };
  window.addEventListener("pointerup", up, true);
  window.addEventListener("pointercancel", cancel, true);
}
