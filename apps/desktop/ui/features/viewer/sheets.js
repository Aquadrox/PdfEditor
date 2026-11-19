// Panneaux latéraux en feuilles (disposition compacte) : Propriétés,
// Assistant IA, Formulaire et Signatures montent du bas de l'écran,
// par-dessus la page. Un bouton (ou un appui sur le titre) les replie sur
// leur seul titre pour laisser voir le document.

import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

/** Panneaux et état de départ (Signatures : replié, la page d'abord). */
const SHEETS = [
  ["properties", false],
  ["ai-panel", false],
  ["form-panel", false],
  ["signatures-panel", true],
];

function svgIcon(id) {
  const ns = "http://www.w3.org/2000/svg";
  const svg = document.createElementNS(ns, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(ns, "use");
  use.setAttribute("href", id);
  svg.append(use);
  return svg;
}

function setCollapsed(panel, button, collapsed) {
  panel.classList.toggle("collapsed", collapsed);
  button.setAttribute("aria-expanded", String(!collapsed));
  const label = t(collapsed ? "sheet.expand" : "sheet.collapse");
  button.setAttribute("aria-label", label);
  button.title = label;
  button.querySelector("use").setAttribute("href", collapsed ? "#i-up" : "#i-down");
}

export function attachSheets() {
  for (const [id, collapsed] of SHEETS) {
    const panel = byId(id);
    const header = panel.querySelector(".properties-header");
    const button = document.createElement("button");
    button.type = "button";
    button.className = "btn ghost icon sheet-toggle";
    button.setAttribute("aria-controls", id);
    button.append(svgIcon("#i-down"));
    header.append(button);
    setCollapsed(panel, button, collapsed);
    const toggle = () => setCollapsed(panel, button, !panel.classList.contains("collapsed"));
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      toggle();
    });
    // Titre de la feuille : grande cible pour le pouce (les autres boutons
    // de l'en-tête gardent leur rôle).
    header.addEventListener("click", (event) => {
      if (document.documentElement.classList.contains("compact") && !event.target.closest("button, a, input, select")) toggle();
    });
  }
}

