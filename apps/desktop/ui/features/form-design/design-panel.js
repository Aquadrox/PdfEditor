// Panneau du mode Formulaire (E-12) : réglages du champ choisi — nom,
// libellé, format intelligent (date, montant, téléphone…), calcul, options
// d'une liste, valeur d'un bouton, obligatoire, lecture seule, apparence.
// Chaque changement renvoie le champ entier à Rust (set_form_field), qui
// l'applique à tous ses emplacements en une opération annulable.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { DATE_PATTERNS, DEFAULT_LOOK, formatChoice, formatFor, kindKey, parseOptions, templateOf } from "./design-state.js";

const GRAY = { r: 0x80, g: 0x8a, b: 0x96 };
const PAPER = { r: 0xf2, g: 0xf5, b: 0xff };

export class DesignPanel {
  /**
   * @param {object} hooks
   * @param {(name: string, template: object) => void} hooks.onChange
   * @param {(field: object) => void} hooks.onDuplicate
   * @param {(field: object) => void} hooks.onDelete
   * @param {(field: object) => void} hooks.onAddRadio
   */
  constructor(hooks) {
    this.hooks = hooks;
    this.field = null;
    this.fields = [];
    const form = byId("form-props");
    form.addEventListener("submit", (event) => event.preventDefault());
    form.addEventListener("change", (event) => {
      if (event.target.id === "form-format") this.showFormat(formatFor(event.target.value, this.field?.kind.format));
      this.changed();
    });
    byId("form-duplicate").addEventListener("click", () => this.field && hooks.onDuplicate(this.field));
    byId("form-delete").addEventListener("click", () => this.field && hooks.onDelete(this.field));
    byId("form-add-radio").addEventListener("click", () => this.field && hooks.onAddRadio(this.field));
    byId("form-date-pattern").replaceChildren(...DATE_PATTERNS.map(([p, sample]) => el("option", { value: p }, `${sample} (${p})`)));
  }

  /** Champ choisi (null : aucun) ; `fields` : tous, pour les calculs. */
  show(field, fields) {
    this.field = field;
    this.fields = fields;
    byId("form-empty").hidden = Boolean(field);
    byId("form-props").hidden = !field;
    if (!field) return;
    const kind = field.kind.kind;
    byId("form-kind").textContent = t(kindKey(field));
    byId("form-name").value = field.name;
    byId("form-tooltip").value = field.tooltip ?? "";
    byId("form-text-group").hidden = kind !== "text";
    byId("form-choice-group").hidden = kind !== "choice";
    byId("form-button-group").hidden = kind !== "checkbox" && kind !== "radio";
    if (kind === "text") this.showText(field);
    if (kind === "choice") {
      byId("form-options").value = field.kind.options.join("\n");
      byId("form-editable").checked = field.kind.editable;
      byId("form-editable").closest("label").hidden = field.kind.list;
      byId("form-multi").checked = field.kind.multi;
      byId("form-multi").closest("label").hidden = !field.kind.list;
    }
    if (kind === "checkbox" || kind === "radio") {
      const exportInput = byId("form-export");
      exportInput.value = field.widgets[0]?.export ?? "";
      // Valeur d'un bouton radio : propre à chaque bouton, fixée à sa création.
      exportInput.disabled = kind === "radio" || !field.created;
      byId("form-add-radio").hidden = kind !== "radio" || !field.created;
    }
    const look = field.look ?? DEFAULT_LOOK;
    byId("form-required").checked = field.required;
    byId("form-read-only").checked = field.read_only;
    byId("form-font-size").value = String(look.font_size || 0);
    byId("form-align").value = look.align;
    byId("form-border").checked = Boolean(look.border);
    byId("form-background").checked = Boolean(look.background);
  }

  showText(field) {
    const k = field.kind;
    byId("form-multiline").checked = k.multiline;
    byId("form-max-len").value = k.max_len ?? "";
    byId("form-comb").checked = k.comb;
    this.showFormat(k.format);
    byId("form-calc-op").value = k.calc?.op ?? "";
    const names = this.fields.filter((f) => f.name !== field.name && f.kind.kind === "text").map((f) => f.name);
    const box = byId("form-calc-fields");
    box.replaceChildren(
      ...names.map((name) =>
        el(
          "label",
          { class: "inline checkbox" },
          el("input", { type: "checkbox", value: name, checked: k.calc?.fields.includes(name) || undefined }),
          el("span", {}, name),
        ),
      ),
    );
    box.hidden = !k.calc && byId("form-calc-op").value === "";
  }

  /** Réglages visibles selon le format choisi. */
  showFormat(format) {
    const choice = formatChoice(format);
    byId("form-format").value = choice;
    const patterns = byId("form-date-pattern");
    patterns.hidden = choice !== "date";
    if (choice === "date") {
      if (![...patterns.options].some((o) => o.value === format.pattern)) patterns.append(el("option", { value: format.pattern }, format.pattern));
      patterns.value = format.pattern;
    }
    const numeric = format?.kind === "number" || format?.kind === "percent";
    byId("form-number-group").hidden = !numeric;
    if (numeric) {
      byId("form-decimals").value = String(format.decimals);
      byId("form-currency").value = format.currency ?? "";
      byId("form-currency").closest("label").hidden = format.kind === "percent";
    }
    byId("form-mask").hidden = choice !== "mask";
    if (format?.kind === "mask") byId("form-mask").value = format.mask;
    byId("form-format-hint").textContent = choice === "none" ? "" : t(`formDesign.formatHint.${choice}`);
  }

  /** Format lu dans les contrôles. */
  readFormat() {
    let format = formatFor(byId("form-format").value, this.field.kind.format);
    if (!format) return null;
    if (format.kind === "date") format = { ...format, pattern: byId("form-date-pattern").value || format.pattern };
    if (format.kind === "number" || format.kind === "percent") {
      const decimals = Math.min(Math.max(Number.parseInt(byId("form-decimals").value, 10) || 0, 0), 10);
      format = { ...format, decimals };
      if (format.kind === "number") format.currency = byId("form-currency").value;
    }
    if (format.kind === "mask" && byId("form-format").value === "mask") format = { ...format, mask: byId("form-mask").value.trim() };
    return format;
  }

  /** Genre lu dans les contrôles. */
  readKind() {
    const k = this.field.kind;
    if (k.kind === "text") {
      const maxLen = Number.parseInt(byId("form-max-len").value, 10);
      const op = byId("form-calc-op").value;
      const chosen = [...byId("form-calc-fields").querySelectorAll("input:checked")].map((i) => i.value);
      return {
        ...k,
        multiline: byId("form-multiline").checked,
        max_len: Number.isFinite(maxLen) && maxLen > 0 ? maxLen : null,
        comb: byId("form-comb").checked,
        format: this.readFormat(),
        calc: op && chosen.length > 0 ? { op, fields: chosen } : null,
      };
    }
    if (k.kind === "choice") {
      return { ...k, options: parseOptions(byId("form-options").value), editable: byId("form-editable").checked && !k.list, multi: byId("form-multi").checked && k.list };
    }
    return k;
  }

  changed() {
    const field = this.field;
    if (!field) return;
    if (field.kind.kind === "text") byId("form-calc-fields").hidden = byId("form-calc-op").value === "";
    const size = Number(byId("form-font-size").value);
    const look = {
      ...(field.look ?? DEFAULT_LOOK),
      font_size: Number.isFinite(size) && size >= 4 ? Math.min(size, 72) : 0,
      align: byId("form-align").value,
      border: byId("form-border").checked ? (field.look?.border ?? GRAY) : null,
      background: byId("form-background").checked ? (field.look?.background ?? PAPER) : null,
    };
    const patch = {
      name: byId("form-name").value.trim() || field.name,
      tooltip: byId("form-tooltip").value.trim(),
      field_kind: this.readKind(),
      required: byId("form-required").checked,
      read_only: byId("form-read-only").checked,
      look,
    };
    if (field.kind.kind === "checkbox" && field.created) patch.export = byId("form-export").value.trim() || "Oui";
    this.hooks.onChange(field.name, templateOf(field, patch));
  }
}
