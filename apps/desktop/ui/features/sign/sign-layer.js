// Couche de signature posée sur chaque page en mode Signer : cadres des
// champs de signature (vides : à cliquer pour signer) et tracé du cadre
// d'une nouvelle signature visible.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { pagePoint } from "../edit/edit-state.js";
import { percentBox } from "../viewer/text-geometry.js";
import { frameAt, frameBetween } from "./sign-state.js";

/** Au-delà de ce déplacement (points), le geste est un tracé, pas un clic. */
const CLICK_TOLERANCE = 4;

/**
 * Remplit `frame` (page `index`) : `signatures` à montrer, `placing` pour
 * tracer un cadre. `onField(signature)` : champ vide cliqué ;
 * `onFrame(cadre)` : cadre tracé (points, origine en haut).
 */
export function renderSignLayer(frame, index, page, { signatures, placing, onField, onFrame }) {
  frame.querySelector(".sign-layer")?.remove();
  const layer = el("div", { class: placing ? "sign-layer placing" : "sign-layer" });
  for (const signature of signatures.filter((s) => s.page === index && s.bounds)) {
    const empty = signature.state === "empty";
    const box = el(
      empty ? "button" : "div",
      {
        class: `sig-box ${signature.state}`,
        type: empty ? "button" : undefined,
        "aria-label": empty ? `${t("sig.signHere")} : ${signature.name}` : t(`sig.state.${signature.state}`),
        title: empty ? t("sig.signHere") : `${signature.signer ?? signature.name} · ${t(`sig.state.${signature.state}`)}`,
      },
      empty ? el("span", { class: "sig-box-label" }, t("sig.signHere")) : "",
    );
    if (empty) box.addEventListener("click", () => onField(signature));
    Object.assign(box.style, percentBox(signature.bounds, page));
    layer.append(box);
  }
  if (placing) layer.addEventListener("pointerdown", (event) => draw(event, layer, page, onFrame));
  frame.append(layer);
}

function draw(event, layer, page, onFrame) {
  if (event.button !== 0 || event.target !== layer) return;
  event.preventDefault();
  const rect = layer.getBoundingClientRect();
  const toPage = (e) => pagePoint(e.clientX - rect.left, e.clientY - rect.top, rect.width, rect.height, page);
  const start = toPage(event);
  let end = start;
  const draft = el("div", { class: "sig-draft", "aria-hidden": "true" });
  layer.append(draft);
  const show = () => {
    const box = frameBetween(start, end, page) ?? { x: start.x, y: start.y, width: 0, height: 0 };
    Object.assign(draft.style, percentBox(box, page));
  };
  const finish = (frameBox) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key, true);
    draft.remove();
    if (frameBox) onFrame(frameBox);
  };
  const move = (e) => {
    end = toPage(e);
    show();
  };
  const up = () => {
    const moved = Math.hypot(end.x - start.x, end.y - start.y) > CLICK_TOLERANCE;
    finish(moved ? frameBetween(start, end, page) : frameAt(start, page));
  };
  const key = (e) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    finish(null);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key, true);
  show();
}
