// Formulaire (E-08) en mode Lire : un contrôle transparent posé sur chaque
// champ (texte, case, bouton radio, liste). La valeur visible est celle
// dessinée par PDFium dans la page ; le texte d'un champ n'apparaît dans
// son contrôle que pendant la saisie. Chaque valeur est une opération
// annulable ; « Aplatir le formulaire » est dans le menu.

import { toast } from "../../components/toast.js";
import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { percentBox } from "../viewer/text-geometry.js";
import { clicked, currentValue, isOn, textOf, typed, widgetsOn } from "./form-state.js";

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

  /** Mode Lire : les champs se remplissent. */
  setActive(active) {
    this.active = active;
    this.refresh();
  }

  get usable() {
    return this.fields.length > 0 && !this.session.view.flattened;
  }

  refresh() {
    byId("menu-flatten").disabled = !this.doc || !this.usable;
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
    for (const { field, widget } of items) layer.append(this.control(field, widget, page));
    frame.append(layer);
  }

  control(field, widget, page) {
    const value = currentValue(field, this.session.view.fields);
    const kind = field.kind.kind;
    let node;
    if (kind === "checkbox" || kind === "radio") {
      node = el("button", {
        class: "form-field toggle",
        type: "button",
        role: kind,
        "aria-checked": String(isOn(field, widget, value)),
        onclick: () => this.set(field, clicked(field, widget, value)),
      });
    } else if (kind === "choice") {
      node = el("select", { class: "form-field choice" });
      const options = value && !field.kind.options.includes(textOf(value)) ? [textOf(value), ...field.kind.options] : field.kind.options;
      node.append(el("option", { value: "" }, ""), ...options.map((o) => el("option", { value: o }, o)));
      node.value = textOf(value);
      node.addEventListener("change", () => this.set(field, node.value ? typed(field, node.value) : null));
    } else {
      node = el(field.kind.multiline ? "textarea" : "input", { class: "form-field text", spellcheck: "false" });
      if (field.kind.max_len) node.maxLength = field.kind.max_len;
      node.value = textOf(value);
      node.addEventListener("change", () => this.set(field, typed(field, node.value)));
      node.addEventListener("keydown", (event) => {
        if (event.key === "Enter" && !field.kind.multiline) node.blur();
        if (event.key === "Escape") {
          node.value = textOf(value);
          node.blur();
        }
      });
    }
    node.setAttribute("aria-label", field.name);
    node.disabled = field.read_only;
    Object.assign(node.style, percentBox(widget.bounds, page));
    return node;
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
