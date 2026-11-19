// Formulaire (E-08), sans DOM : valeur affichée d'un champ (saisie, sinon
// celle du document), état d'une case ou d'un bouton radio, valeur après un
// clic, champs d'une page. Les valeurs suivent le format de pde-core :
// { kind: "text" | "checked" | "choice" | "choices", value }.

import { displayValue } from "../form-design/design-state.js";

/** Valeur courante : celle saisie (EditView.fields), sinon celle d'origine. */
export function currentValue(field, edited) {
  return edited?.[field.name] ?? field.value ?? null;
}

/** Texte d'un champ texte ou d'une liste (première option d'un choix multiple). */
export function textOf(value) {
  if (value?.kind === "choices") return value.value[0] ?? "";
  return value && (value.kind === "text" || value.kind === "choice") ? value.value : "";
}

/**
 * Pourquoi le champ ne se remplit pas ici : "readOnly", "calculated",
 * "password" ; null s'il se remplit.
 */
export function blocked(field) {
  if (field.read_only) return "readOnly";
  if (field.kind.kind === "text" && field.kind.calc) return "calculated";
  if (field.kind.kind === "text" && field.kind.password) return "password";
  return null;
}

/** Exemple de saisie d'un champ mis en forme (date, montant, téléphone…). */
export function hint(field) {
  const format = field.kind.kind === "text" ? field.kind.format : null;
  if (!format) return "";
  switch (format.kind) {
    case "date":
      return format.pattern.replace(/yyyy/g, "aaaa").replace(/yy/g, "aa").replace(/dd?/g, "jj").replace(/mmmm|mmm/g, "mois");
    case "time":
      return format.pattern;
    case "number":
    case "percent":
      return displayValue(format, "1234.5");
    case "mask":
      return format.mask.replace(/9/g, "0");
    case "email":
      return "nom@exemple.fr";
    default:
      return "";
  }
}

/** Case cochée, ou bouton radio de cet emplacement choisi. */
export function isOn(field, widget, value) {
  if (field.kind.kind === "checkbox") return value?.kind === "checked" && value.value === true;
  if (field.kind.kind === "radio") return value?.kind === "choice" && value.value === field.kind.options[widget.option];
  return false;
}

/** Valeur après un clic sur une case ou un bouton radio. */
export function clicked(field, widget, value) {
  if (field.kind.kind === "checkbox") return { kind: "checked", value: !isOn(field, widget, value) };
  return { kind: "choice", value: field.kind.options[widget.option] };
}

/** Valeur saisie dans un champ texte ou une liste. */
export function typed(field, text) {
  return { kind: field.kind.kind === "text" ? "text" : "choice", value: text };
}

/** Emplacements à remplir de la page `page` : [{ field, widget }] (les signatures se font en mode Signer). */
export function widgetsOn(fields, page) {
  return fields
    .filter((field) => field.kind.kind !== "signature")
    .flatMap((field) => field.widgets.filter((w) => w.page === page).map((widget) => ({ field, widget })));
}
