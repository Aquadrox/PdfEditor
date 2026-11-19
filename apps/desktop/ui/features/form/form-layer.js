// Formulaire (E-08) en mode Lire : un contrôle transparent posé sur chaque
// champ (texte, case, bouton radio, liste déroulante ou affichée). La
// valeur visible est celle dessinée dans la page ; le texte d'un champ
// n'apparaît dans son contrôle que pendant la saisie. Un champ « intelligent »
// (date, montant, téléphone…) est vérifié et mis en forme par Rust ; une
// date se choisit aussi dans un calendrier. Champs obligatoires signalés,
// champs calculés et mots de passe non modifiables. Chaque valeur est une
// opération annulable ; « Aplatir le formulaire » est dans le menu.

import { toast } from "../../components/toast.js";
import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { percentBox } from "../viewer/text-geometry.js";
import { blocked, clicked, currentValue, hint, isOn, textOf, typed, widgetsOn } from "./form-state.js";

const SVG = "http://www.w3.org/2000/svg";

/** Icône du jeu de l'interface (élément SVG, aucun HTML). */
function icon(id) {
  const svg = document.createElementNS(SVG, "svg");
  svg.setAttribute("class", "icon");
  svg.setAttribute("aria-hidden", "true");
  const use = document.createElementNS(SVG, "use");
  use.setAttribute("href", `#${id}`);
  svg.append(use);
  return svg;
}

const fail = (error) => toast(t("form.failed", { message: error?.message ?? String(error) }), "error");

export class FormLayer {
  /**
   * @param {import("../edit/edit-session.js").EditSession} session
   * @param {import("../viewer/viewer.js").Viewer} viewer
   */
  constructor(session, viewer) {
    this.session = session;
    this.viewer = viewer;
    this.doc = null;
    this.fields = [];
    this.active = false;
    /** Aplatir est refusé sur un document verrouillé par sa signature. */
    this.flattenAllowed = true;
    session.subscribe(() => this.refresh());
    byId("menu-flatten").addEventListener("click", () => this.flatten());
  }

  /** Champs du document (relus quand ses pages changent). */
  async load(doc) {
    this.doc = doc;
    this.fields = [];
    this.refresh();
    if (!doc) return;
    const fields = await invoke("form_fields", { id: doc.id }).catch(() => []);
    if (this.doc !== doc) return;
    this.fields = fields ?? [];
    this.refresh();
  }

  /** Champs relus après une conception (mode Formulaire). */
  setFields(fields) {
    this.fields = fields ?? [];
    this.refresh();
  }

  /** Mode Lire : les champs se remplissent. */
  setActive(active) {
    this.active = active;
    this.refresh();
  }

  get usable() {
    return this.fields.length > 0 && !this.session.view.flattened;
  }

  refresh() {
    byId("menu-flatten").disabled = !this.doc || !this.usable || !this.flattenAllowed;
    this.viewer.frames.forEach((frame, index) => {
      if (frame.querySelector("img")) this.render(frame, index);
    });
  }

  /** Page affichée par la visionneuse : ses champs. */
  pageShown(frame, index) {
    if (!frame.querySelector(".form-layer")) this.render(frame, index);
  }

  render(frame, index) {
    const focused = frame.querySelector(".form-layer :focus");
    // Pas de redessin pendant une saisie : il effacerait le texte en cours.
    if (focused && focused.classList.contains("text")) return;
    frame.querySelector(".form-layer")?.remove();
    if (!this.active || !this.usable || !this.viewer.doc) return;
    const items = widgetsOn(this.fields, index);
    if (items.length === 0) return;
    const page = this.viewer.doc.pages[index];
    const layer = el("div", { class: "form-layer" });
    for (const { field, widget } of items) layer.append(...this.controls(field, widget, page));
    frame.append(layer);
  }

  /** Contrôle d'un emplacement (et le bouton calendrier d'une date). */
  controls(field, widget, page) {
    const value = currentValue(field, this.session.view.fields);
    const kind = field.kind.kind;
    let node;
    const extra = [];
    if (kind === "checkbox" || kind === "radio") {
      node = el("button", {
        class: "form-field toggle",
        type: "button",
        role: kind,
        "aria-checked": String(isOn(field, widget, value)),
        onclick: () => this.set(field, clicked(field, widget, value)),
      });
    } else if (kind === "choice" && field.kind.editable) {
      node = this.textInput(field, value);
      const list = el("datalist", { id: `form-options-${field.name}-${widget.index}` }, ...field.kind.options.map((o) => el("option", { value: o })));
      node.setAttribute("list", list.id);
      extra.push(list);
    } else if (kind === "choice") {
      node = el("select", { class: field.kind.list ? "form-field choice list" : "form-field choice" });
      if (field.kind.list) node.size = Math.max(2, field.kind.options.length);
      node.multiple = Boolean(field.kind.multi);
      const chosen = value?.kind === "choices" ? value.value : [textOf(value)];
      const options = field.kind.options.slice();
      for (const c of chosen) if (c && !options.includes(c)) options.unshift(c);
      if (!field.kind.list) node.append(el("option", { value: "" }, ""));
      node.append(...options.map((o) => el("option", { value: o, selected: chosen.includes(o) || undefined }, o)));
      if (!node.multiple) node.value = textOf(value);
      node.addEventListener("change", () => {
        const picked = [...node.selectedOptions].map((o) => o.value).filter(Boolean);
        this.set(field, node.multiple ? { kind: "choices", value: picked } : node.value ? typed(field, node.value) : null);
      });
    } else {
      node = this.textInput(field, value);
      if (field.kind.format?.kind === "date" && !blocked(field)) extra.push(this.datePicker(field, widget, page));
    }
    node.setAttribute("aria-label", field.name);
    const reason = blocked(field);
    node.disabled = Boolean(reason);
    const help = [field.tooltip, reason ? t(`form.blocked.${reason}`) : hint(field)].filter(Boolean).join(" · ");
    if (help) node.title = help;
    if (field.required) {
      node.classList.add("required");
      node.setAttribute("aria-required", "true");
    }
    Object.assign(node.style, percentBox(widget.bounds, page));
    return [node, ...extra];
  }

  textInput(field, value) {
    const multiline = field.kind.kind === "text" && field.kind.multiline;
    const node = el(multiline ? "textarea" : "input", { class: "form-field text", spellcheck: "false" });
    if (field.kind.max_len) node.maxLength = field.kind.max_len;
    if (field.kind.password) node.type = "password";
    node.value = textOf(value);
    const example = hint(field);
    if (example) node.placeholder = example;
    node.addEventListener("change", () => this.set(field, typed(field, node.value)));
    node.addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !multiline) node.blur();
      if (event.key === "Escape") {
        node.value = textOf(value);
        node.blur();
      }
    });
    return node;
  }

  /** Bouton calendrier au bord droit d'un champ date ; la date choisie part en ISO, Rust l'écrit au format du champ. */
  datePicker(field, widget, page) {
    const picker = el("input", { type: "date", class: "form-date-input", tabindex: "-1", "aria-hidden": "true" });
    picker.addEventListener("change", () => picker.value && this.set(field, { kind: "text", value: picker.value }));
    const button = el(
      "button",
      { class: "form-date", type: "button", "aria-label": t("form.pickDate", { name: field.name }), title: t("form.pickDate", { name: field.name }) },
      picker,
    );
    button.prepend(icon("i-calendar"));
    button.addEventListener("click", () => {
      try {
        picker.showPicker();
      } catch {
        picker.focus();
      }
    });
    const b = widget.bounds;
    const side = Math.min(b.height, 16);
    Object.assign(button.style, percentBox({ x: b.x + b.width + 2, y: b.y + (b.height - side) / 2, width: side, height: side }, page));
    return button;
  }

  async set(field, value) {
    if (!this.doc) return;
    try {
      const view = await invoke("set_field", { id: this.doc.id, name: field.name, value });
      await this.session.apply(view);
    } catch (error) {
      fail(error);
      this.refresh();
    }
  }

  async flatten() {
    byId("main-menu").hidden = true;
    if (!this.doc || !this.usable) return;
    try {
      await this.session.apply(await invoke("set_flatten", { id: this.doc.id, flatten: true }));
      toast(t("form.flattened"));
    } catch (error) {
      fail(error);
    }
  }
}
