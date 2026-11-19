// Mode Formulaire (E-12) : créer un formulaire sur une page vierge, un PDF
// ou un document Word importé. Outils de champs (texte, zone, date,
// montant, e-mail, téléphone, case, choix, liste, signature), calque de
// conception sur les pages, panneau de réglages, détection des champs d'un
// formulaire « papier », « Tester » pour le remplir en mode Lire. Chaque
// action passe par pde-app et s'annule (Ctrl+Z).

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { renderDesignLayer } from "./design-layer.js";
import { DesignPanel } from "./design-panel.js";
import { boxAt, clampBox, fieldSignature, freeExport, newField } from "./design-state.js";

const fail = (error) => toast(t("form.failed", { message: error?.message ?? String(error) }), "error");

export class FormDesign {
  /**
   * @param {import("../edit/edit-session.js").EditSession} session
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {object} hooks
   * @param {() => void} hooks.test passer en mode Lire pour remplir
   * @param {() => void} hooks.fieldsChanged les champs ont changé (mode Lire)
   */
  constructor(session, viewer, hooks) {
    this.session = session;
    this.viewer = viewer;
    this.hooks = hooks;
    this.doc = null;
    this.fields = [];
    this.active = false;
    this.tool = "select";
    this.selected = null;
    this.suggestions = null;
    this.signature = "";
    this.panel = new DesignPanel({
      onChange: (name, template) => this.run("set_form_field", { name, field: template }),
      onDuplicate: (field) => this.duplicate(field),
      onDelete: (field) => this.remove(field, null),
      onAddRadio: (field) => this.addRadio(field),
    });
    for (const button of document.querySelectorAll("[data-field-tool]")) {
      button.addEventListener("click", () => this.setTool(button.dataset.fieldTool));
    }
    byId("form-detect").addEventListener("click", () => this.detect());
    byId("form-test").addEventListener("click", () => hooks.test());
    byId("form-undo").addEventListener("click", () => this.session.doc && this.session.undo().catch(fail));
    byId("form-redo").addEventListener("click", () => this.session.doc && this.session.redo().catch(fail));
    byId("form-suggestions-add").addEventListener("click", () => this.addSuggestions());
    byId("form-suggestions-cancel").addEventListener("click", () => this.setSuggestions(null));
    session.subscribe((view) => {
      byId("form-undo").disabled = !view.can_undo;
      byId("form-redo").disabled = !view.can_redo;
      // Champs conçus changés (action, annuler, rétablir) : liste relue.
      const signature = fieldSignature(view);
      if (signature !== this.signature) {
        this.signature = signature;
        this.reload();
      } else this.refresh();
    });
  }

  async load(doc) {
    this.doc = doc;
    this.fields = [];
    this.selected = null;
    this.suggestions = null;
    this.signature = fieldSignature(this.session.view);
    this.refresh();
    if (doc) await this.reload();
  }

  /** Champs relus depuis Rust (créés, modifiés, du document). */
  async reload() {
    const doc = this.doc;
    if (!doc) return;
    const fields = await invoke("form_fields", { id: doc.id }).catch(() => []);
    if (this.doc !== doc) return;
    this.fields = fields ?? [];
    if (this.selected && !this.selectedField()) this.selected = null;
    this.refresh();
    this.hooks.fieldsChanged(this.fields);
  }

  setActive(active) {
    this.active = active;
    byId("form-toolbar").hidden = !active;
    byId("form-panel").hidden = !active;
    if (!active) {
      this.tool = "select";
      this.setSuggestions(null);
    }
    this.refresh();
  }

  setTool(tool) {
    this.tool = tool;
    for (const button of document.querySelectorAll("[data-field-tool]")) {
      const on = button.dataset.fieldTool === tool;
      button.classList.toggle("on", on);
      button.classList.toggle("ghost", !on);
      button.setAttribute("aria-pressed", String(on));
    }
    this.refresh();
  }

  selectedField() {
    return this.selected ? this.fields.find((f) => f.name === this.selected.name) : null;
  }

  refresh() {
    const count = this.fields.length;
    byId("form-count").textContent = this.doc ? t(count === 1 ? "formDesign.countOne" : "formDesign.count", { count }) : "";
    if (this.active) this.panel.show(this.selectedField() ?? null, this.fields);
    this.viewer.frames.forEach((frame, index) => {
      if (frame.querySelector("img")) this.render(frame, index);
    });
  }

  pageShown(frame, index) {
    if (!frame.querySelector(".design-layer")) this.render(frame, index);
  }

  render(frame, index) {
    frame.querySelector(".design-layer")?.remove();
    if (!this.active || !this.viewer.doc) return;
    const page = this.viewer.doc.pages[index];
    renderDesignLayer(
      frame,
      index,
      page,
      { fields: this.fields, selected: this.selected, tool: this.tool, suggestions: this.suggestions },
      {
        onSelect: (field, widget) => {
          this.selected = field ? { name: field.name, index: widget.index } : null;
          this.refresh();
          // Cadre redessiné : il garde le focus (Suppr, flèches).
          frame.querySelector(".design-box.selected")?.focus();
        },
        onMove: (field, widget, box) => {
          this.selected = { name: field.name, index: widget.index };
          this.run("place_form_widgets", {
            places: [{ name: field.name, index: widget.index, page: widget.page, top_left: { x: box.x, y: box.y }, width: box.width, height: box.height }],
          });
        },
        onCreate: (pageIndex, shape) => this.create(pageIndex, shape),
        onDelete: (field, widget) => this.remove(field, widget),
      },
    );
  }

  /** Appelle une commande de conception et applique l'état rendu. */
  async run(command, args) {
    if (!this.doc) return null;
    try {
      const result = await invoke(command, { id: this.doc.id, ...args });
      await this.session.apply(result.view ?? result);
      return result;
    } catch (error) {
      fail(error);
      this.refresh();
      return null;
    }
  }

  /** Nouveau champ de l'outil choisi, tracé (`box`) ou d'un clic (`point`). */
  async create(pageIndex, shape) {
    const page = this.viewer.doc.pages[pageIndex];
    const box = shape.box ?? boxAt(shape.point, this.tool, page);
    const field = newField(this.tool, box, pageIndex, this.fields.map((f) => f.name));
    await this.addFields([field]);
    this.setTool("select");
  }

  /** Ajoute des champs et choisit le premier créé. */
  async addFields(fields) {
    const result = await this.run("add_form_fields", { fields });
    if (!result) return;
    await this.reload();
    const id = result.ids?.[0];
    const found = this.fields.find((f) => f.widgets.some((w) => w.element === id));
    const widget = found?.widgets.find((w) => w.element === id);
    if (found && widget) this.selected = { name: found.name, index: widget.index };
    this.refresh();
  }

  /** Copie du champ sous l'original (nom libre donné par Rust). */
  duplicate(field) {
    const widget = field.widgets.find((w) => w.index === this.selected?.index) ?? field.widgets[0];
    if (!widget) return;
    const page = this.viewer.doc.pages[widget.page];
    const b = widget.bounds;
    const box = clampBox({ ...b, y: b.y + b.height + 8 }, page);
    const kind = field.kind.kind === "radio" ? { kind: "radio", options: [] } : field.kind;
    const copy = {
      ...newField("text", box, widget.page, []),
      name: field.name,
      field_kind: kind,
      tooltip: field.tooltip,
      required: field.required,
      read_only: field.read_only,
      look: field.look,
      export: widget.export ?? (field.kind.kind === "checkbox" ? "Oui" : ""),
    };
    if (field.kind.kind === "radio") copy.name = `${field.name}_copie`;
    return this.addFields([copy]);
  }

  /** Bouton de plus dans un groupe radio créé, à droite du dernier. */
  addRadio(field) {
    const last = field.widgets[field.widgets.length - 1];
    if (!last) return;
    const page = this.viewer.doc.pages[last.page];
    const box = clampBox({ ...last.bounds, x: last.bounds.x + last.bounds.width + 24 }, page);
    const exports = field.widgets.map((w) => w.export ?? "");
    const button = newField("radio", box, last.page, [], { name: field.name, exports });
    button.export = freeExport(exports);
    return this.addFields([button]);
  }

  /** Supprime un emplacement (`widget`) ou le champ entier. */
  async remove(field, widget) {
    const index = widget && field.widgets.length > 1 ? widget.index : null;
    const done = await this.run("delete_form_widgets", { targets: [{ name: field.name, index }] });
    if (done) this.selected = null;
  }

  /** Champs proposés pour la page affichée. */
  async detect() {
    if (!this.doc) return;
    const page = this.viewer.page;
    try {
      const found = (await invoke("suggest_form_fields", { id: this.doc.id, page })) ?? [];
      if (found.length === 0) {
        toast(t("formDesign.nothingFound", { page: page + 1 }));
        this.setSuggestions(null);
        return;
      }
      this.setSuggestions(found.map((field) => ({ field, chosen: true })));
    } catch (error) {
      fail(error);
    }
  }

  setSuggestions(suggestions) {
    this.suggestions = suggestions;
    byId("form-suggestions").hidden = !suggestions;
    if (suggestions) {
      byId("form-suggestions-text").textContent = t("formDesign.suggestionsText", { count: suggestions.length });
      const list = byId("form-suggestions-list");
      list.replaceChildren(
        ...suggestions.map((s) => {
          const input = document.createElement("input");
          input.type = "checkbox";
          input.checked = s.chosen;
          input.addEventListener("change", () => {
            s.chosen = input.checked;
            this.refresh();
          });
          const label = document.createElement("label");
          label.className = "inline checkbox";
          const text = document.createElement("span");
          text.textContent = s.field.tooltip ? `${s.field.tooltip} · ${s.field.name}` : s.field.name;
          label.append(input, text);
          const item = document.createElement("li");
          item.append(label);
          return item;
        }),
      );
    }
    this.refresh();
  }

  async addSuggestions() {
    const chosen = (this.suggestions ?? []).filter((s) => s.chosen).map((s) => s.field);
    this.setSuggestions(null);
    if (chosen.length === 0) return;
    await this.addFields(chosen);
    toast(t("formDesign.added", { count: chosen.length }));
  }
}
