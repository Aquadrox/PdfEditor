// Tiroir des pages et du plan (disposition compacte) : la barre latérale
// glisse par-dessus la page, un voile ferme le tiroir, comme le Retour
// d'Android ou le choix d'une page.

import { onBack } from "../../lib/back.js";
import { isCompact, onCompactChange } from "../../lib/compact.js";
import { byId } from "../../lib/dom.js";

export function attachSidebarDrawer() {
  const sidebar = byId("sidebar");
  const scrim = byId("scrim");
  const toggle = byId("sidebar-toggle");
  let release = null;

  function setOpen(open) {
    if (open === sidebar.classList.contains("open")) return;
    sidebar.classList.toggle("open", open);
    scrim.hidden = !open;
    toggle.setAttribute("aria-expanded", String(open));
    if (open) {
      release = onBack(() => {
        release = null;
        setOpen(false);
      });
      // Page affichée visible dans la liste, focus dans le tiroir.
      sidebar.querySelector('.thumb[aria-current="page"]')?.scrollIntoView({ block: "center" });
      byId("sidebar-body").focus({ preventScroll: true });
    } else {
      release?.();
      release = null;
    }
  }

  toggle.addEventListener("click", () => setOpen(!sidebar.classList.contains("open")));
  scrim.addEventListener("click", () => setOpen(false));
  // Une page ou un titre du plan choisi : le tiroir laisse voir la page.
  sidebar.addEventListener("click", (event) => {
    if (isCompact() && event.target.closest(".thumb, .outline-item")) setOpen(false);
  });
  sidebar.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && sidebar.classList.contains("open")) {
      event.stopPropagation();
      setOpen(false);
      toggle.focus();
    }
  });
  onCompactChange((compact) => !compact && setOpen(false));

  return {
    /** Document ouvert ou fermé : le bouton suit, le tiroir se referme. */
    setDocument(open) {
      toggle.hidden = !open;
      setOpen(false);
    },
    close: () => setOpen(false),
  };
}
