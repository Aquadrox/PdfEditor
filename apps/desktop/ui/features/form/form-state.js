// Formulaire (E-08), sans DOM : valeur affichée d'un champ (saisie, sinon
// celle du document), état d'une case ou d'un bouton radio, valeur après un
// clic, champs d'une page. Les valeurs suivent le format de pde-core :
// { kind: "text" | "checked" | "choice", value }.

/** Valeur courante : celle saisie (EditView.fields), sinon celle d'origine. */
export function currentValue(field, edited) {
  return edited?.[field.name] ?? field.value ?? null;
}

/** Texte d'un champ texte ou d'une liste. */
export function textOf(value) {
  return value && (value.kind === "text" || value.kind === "choice") ? value.value : "";
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

/** Emplacements de la page `page` : [{ field, widget }]. */
export function widgetsOn(fields, page) {
  return fields.flatMap((field) => field.widgets.filter((w) => w.page === page).map((widget) => ({ field, widget })));
}
