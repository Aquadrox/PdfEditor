// Panneau « Signatures » (S-07) et sceau de la barre du haut : état de
// chaque signature, en mots, avec une icône (jamais la couleur seule).

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { concern, details, documentLock, sealLabel } from "./sign-state.js";

const ICONS = { valid: "i-check", invalid: "i-alert", unsupported: "i-alert", empty: "i-frame" };

function icon(name) {
  const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS("http://www.w3.org/2000/svg", "use");
  use.setAttribute("href", `#${name}`);
  svg.append(use);
  return svg;
}

/** Liste du panneau ; `onField` : signer un champ vide, `onPage` : aller à la page. */
export function renderPanel(signatures, { onField, onPage }) {
  byId("signatures-empty").hidden = signatures.length > 0;
  const lock = documentLock(signatures);
  byId("signatures-list").replaceChildren(
    ...signatures.map((signature) => {
      const title = signature.state === "empty" ? signature.name : (signature.signer ?? signature.name);
      const lines = details(signature).map(([key, params]) => el("p", { class: "sig-line" }, t(key, params)));
      const actions = [];
      if (signature.state === "empty") {
        actions.push(el("button", { class: "btn sealed small", type: "button", onclick: () => onField(signature) }, t("sig.signHere")));
      } else if (signature.page !== null && signature.page !== undefined) {
        actions.push(el("button", { class: "btn ghost small", type: "button", onclick: () => onPage(signature.page) }, t("sig.goTo")));
      }
      // Valide, mais suivie d'une modification à vérifier ou interdite.
      const worry = signature.state === "valid" ? concern(signature, lock) : "none";
      const iconName = worry === "none" ? ICONS[signature.state] ?? "i-alert" : "i-alert";
      return el(
        "li",
        { class: `sig-item ${signature.state} concern-${worry}` },
        el("span", { class: "sig-icon" }, icon(iconName)),
        el("div", { class: "sig-text" }, el("strong", { class: "sig-name" }, title), el("span", { class: "sig-state" }, t(`sig.state.${signature.state}`)), ...lines, ...actions),
      );
    }),
  );
}

/** Sceau de la barre du haut : caché sans champ de signature. */
export function renderSeal(info) {
  const seal = byId("seal");
  seal.hidden = info.total === 0;
  seal.dataset.tone = info.tone;
  seal.dataset.lock = info.lock ?? "";
  const [key, params] = sealLabel(info);
  byId("seal-label").textContent = t(key, params);
}

/** Le sceau « se presse » une fois à la signature (seule animation appuyée de l'application). */
export function pressSeal() {
  const seal = byId("seal");
  seal.classList.remove("pressed");
  // Relance l'animation même si elle vient d'avoir lieu.
  void seal.offsetWidth;
  seal.classList.add("pressed");
}
