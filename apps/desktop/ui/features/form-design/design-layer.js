// Calque de conception posé sur chaque page en mode Formulaire (E-12) :
// cadre de chaque emplacement de champ (nom, genre), sélection, poignée de
// redimensionnement, déplacement à la souris ou aux flèches, tracé d'un
// nouveau champ avec l'outil choisi, aperçu des champs proposés par la
// détection. Le nom d'un champ vient du document : affiché par textContent.

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { pagePoint, pointsFromPixels } from "../edit/edit-state.js";
import { percentBox } from "../viewer/text-geometry.js";
import { boxesOn, boxBetween, clampBox, kindKey } from "./design-state.js";

/** Au-delà de ce déplacement (points), le geste déplace : ce n'est plus un clic. */
const DRAG_TOLERANCE = 2;

const same = (a, b) => a && b && a.name === b.name && a.index === b.index;

/**
 * Remplit `frame` (page `index`, taille `page` en points).
 * `state` : { fields, selected: { name, index } | null, tool, suggestions }.
 * `hooks` : onSelect(field, widget | null), onMove(field, widget, box),
 * onCreate(page, box | point), onDelete(field, widget).
 */
export function renderDesignLayer(frame, index, page, state, hooks) {
  frame.querySelector(".design-layer")?.remove();
  const drawing = state.tool !== "select";
  const layer = el("div", { class: drawing ? "design-layer drawing" : "design-layer" });
  for (const { field, widget } of boxesOn(state.fields, index)) {
    layer.append(fieldBox(field, widget, page, state, hooks));
  }
  for (const suggestion of state.suggestions ?? []) {
    if (suggestion.field.page !== index) continue;
    const box = el("div", { class: suggestion.chosen ? "design-suggestion chosen" : "design-suggestion", "aria-hidden": "true" });
    const f = suggestion.field;
    Object.assign(box.style, percentBox({ x: f.top_left.x, y: f.top_left.y, width: f.width, height: f.height }, page));
    layer.append(box);
  }
  layer.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.target !== layer) return;
    if (!drawing) {
      hooks.onSelect(null, null);
      return;
    }
    draw(event, layer, page, (shape) => hooks.onCreate(index, shape));
  });
  frame.append(layer);
}

function fieldBox(field, widget, page, state, hooks) {
  const selected = same(state.selected, { name: field.name, index: widget.index });
  const kind = field.kind.kind;
  const box = el(
    "div",
    {
      class: `design-box ${kind}${selected ? " selected" : ""}${field.required ? " required" : ""}`,
      tabindex: "0",
      role: "button",
      "aria-pressed": String(selected),
      "aria-label": `${field.name} · ${t(kindKey(field))}`,
      title: field.tooltip || field.name,
    },
    el("span", { class: "design-label" }, field.name),
  );
  Object.assign(box.style, percentBox(widget.bounds, page));
  box.addEventListener("pointerdown", (event) => {
    if (event.button !== 0 || event.target.classList.contains("design-handle")) return;
    event.stopPropagation();
    drag(event, box, widget, page, (moved) => (moved ? hooks.onMove(field, widget, moved) : hooks.onSelect(field, widget)), "move");
  });
  box.addEventListener("keydown", (event) => keyboard(event, field, widget, page, hooks));
  if (selected) {
    const handle = el("span", { class: "design-handle", "aria-hidden": "true" });
    handle.addEventListener("pointerdown", (event) => {
      if (event.button !== 0) return;
      event.stopPropagation();
      drag(event, box, widget, page, (moved) => moved && hooks.onMove(field, widget, moved), "resize");
    });
    box.append(handle);
  }
  return box;
}

/** Suppr : supprime ; flèches : décale d'un point (Maj : 10). */
function keyboard(event, field, widget, page, hooks) {
  if (event.key === "Delete" || event.key === "Backspace") {
    event.preventDefault();
    hooks.onDelete(field, widget);
    return;
  }
  if (event.key === "Enter" || event.key === " ") {
    event.preventDefault();
    hooks.onSelect(field, widget);
    return;
  }
  const step = event.shiftKey ? 10 : 1;
  const delta = { ArrowLeft: [-step, 0], ArrowRight: [step, 0], ArrowUp: [0, -step], ArrowDown: [0, step] }[event.key];
  if (!delta) return;
  event.preventDefault();
  event.stopPropagation();
  const b = widget.bounds;
  hooks.onMove(field, widget, clampBox({ ...b, x: b.x + delta[0], y: b.y + delta[1] }, page));
}

/** Déplacement ou redimensionnement à la souris ; `done(boîte | null)`. */
function drag(event, box, widget, page, done, mode) {
  event.preventDefault();
  const frameWidth = box.parentElement.getBoundingClientRect().width;
  const start = { x: event.clientX, y: event.clientY };
  let moved = null;
  const move = (e) => {
    const { dx, dy } = pointsFromPixels(e.clientX - start.x, e.clientY - start.y, frameWidth, page);
    if (!moved && Math.hypot(dx, dy) < DRAG_TOLERANCE) return;
    const b = widget.bounds;
    moved = clampBox(
      mode === "move" ? { ...b, x: b.x + dx, y: b.y + dy } : { ...b, width: b.width + dx, height: b.height + dy },
      page,
    );
    Object.assign(box.style, percentBox(moved, page));
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    done(moved);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
}

/** Tracé d'un nouveau champ ; `done({ box } | { point })`. Échap annule. */
function draw(event, layer, page, done) {
  event.preventDefault();
  const rect = layer.getBoundingClientRect();
  const toPage = (e) => pagePoint(e.clientX - rect.left, e.clientY - rect.top, rect.width, rect.height, page);
  const start = toPage(event);
  let end = start;
  const draft = el("div", { class: "design-draft", "aria-hidden": "true" });
  layer.append(draft);
  const finish = (result) => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.removeEventListener("keydown", key, true);
    draft.remove();
    if (result) done(result);
  };
  const move = (e) => {
    end = toPage(e);
    const box = boxBetween(start, end, page);
    if (box) Object.assign(draft.style, percentBox(box, page));
  };
  const up = () => {
    const box = boxBetween(start, end, page);
    finish(box ? { box } : { point: start });
  };
  const key = (e) => {
    if (e.key !== "Escape") return;
    e.stopPropagation();
    finish(null);
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  window.addEventListener("keydown", key, true);
}
