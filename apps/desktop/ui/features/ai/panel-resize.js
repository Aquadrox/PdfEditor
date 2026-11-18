// Largeur d'un panneau latéral réglable : poignée du bord gauche, à la
// souris (glisser) ou au clavier (flèches) ; largeur gardée d'une session à
// l'autre dans ce navigateur (localStorage, sans effet si indisponible).

/** Largeur bornée : au moins `min`, au plus 70 % de la fenêtre. */
export function clampWidth(width, min, windowWidth) {
  const max = Math.max(min, Math.round(windowWidth * 0.7));
  return Math.min(Math.max(Math.round(width), min), max);
}

/**
 * @param {HTMLElement} panel
 * @param {HTMLElement} handle poignée, au bord gauche du panneau
 * @param {string} key clé de mémorisation
 */
export function attachPanelResize(panel, handle, key, min = 300) {
  const apply = (width) => {
    const clamped = clampWidth(width, min, window.innerWidth);
    panel.style.width = `${clamped}px`;
    return clamped;
  };
  const save = (width) => {
    try {
      localStorage.setItem(key, String(width));
    } catch {
      // Stockage indisponible : la largeur vaut pour cette session.
    }
  };
  try {
    const saved = Number(localStorage.getItem(key));
    if (saved > 0) apply(saved);
  } catch {
    // Pas de largeur gardée.
  }

  handle.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    handle.setPointerCapture(event.pointerId);
    handle.classList.add("dragging");
    const startX = event.clientX;
    const startWidth = panel.getBoundingClientRect().width;
    let width = startWidth;
    const move = (e) => {
      // Panneau à droite : tirer vers la gauche l'élargit.
      width = apply(startWidth + (startX - e.clientX));
    };
    const stop = () => {
      handle.classList.remove("dragging");
      handle.removeEventListener("pointermove", move);
      save(width);
    };
    handle.addEventListener("pointermove", move);
    handle.addEventListener("pointerup", stop, { once: true });
    handle.addEventListener("pointercancel", stop, { once: true });
  });
  handle.addEventListener("keydown", (event) => {
    const step = event.shiftKey ? 80 : 20;
    const delta = { ArrowLeft: step, ArrowRight: -step }[event.key];
    if (!delta) return;
    event.preventDefault();
    save(apply(panel.getBoundingClientRect().width + delta));
  });
}
