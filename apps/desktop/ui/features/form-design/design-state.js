// Conception de formulaires (E-12), sans DOM : outils et champs qu'ils
// créent (genre, format, taille par défaut, nom libre), formats proposés
// dans le panneau, réglages d'un champ renvoyés à Rust, place d'un
// emplacement déplacé ou redimensionné dans la page. Les champs suivent le
// format de pde-core (FieldBox, FieldKind, FieldFormat) ; coordonnées en
// points, origine en haut à gauche.

/** Côté minimal d'un champ (comme MIN_FIELD_SIDE de pde-core). */
export const MIN_SIDE = 4;
/** Largeur minimale d'une case d'un champ en cases (MIN_COMB_CELL de pde-core). */
export const MIN_COMB_CELL = 6;

const text = (format = null, extra = {}) => ({
  kind: "text",
  multiline: false,
  max_len: null,
  password: false,
  comb: false,
  format,
  calc: null,
  ...extra,
});

/** Formats proposés : valeur de la liste du panneau → format de pde-core. */
export const FORMATS = {
  none: () => null,
  date: () => ({ kind: "date", pattern: "dd/mm/yyyy" }),
  time: () => ({ kind: "time", pattern: "HH:MM" }),
  number: () => ({ kind: "number", decimals: 0, separator: "comma", currency: "", currency_before: false }),
  amount: () => ({ kind: "number", decimals: 2, separator: "comma", currency: " €", currency_before: false }),
  percent: () => ({ kind: "percent", decimals: 0, separator: "comma" }),
  phone: () => ({ kind: "mask", mask: "99 99 99 99 99" }),
  zip: () => ({ kind: "mask", mask: "99999" }),
  email: () => ({ kind: "email" }),
  mask: () => ({ kind: "mask", mask: "" }),
};

/** Motifs de date proposés (syntaxe d'Acrobat) et leur exemple. */
export const DATE_PATTERNS = [
  ["dd/mm/yyyy", "19/11/2026"],
  ["dd/mm/yy", "19/11/26"],
  ["dd.mm.yyyy", "19.11.2026"],
  ["yyyy-mm-dd", "2026-11-19"],
  ["mm/dd/yyyy", "11/19/2026"],
];

/**
 * Outils de la barre : genre du champ créé, taille d'un simple clic
 * (points) et début de nom.
 */
export const TOOLS = {
  text: { name: "texte", size: [160, 20], kind: () => text() },
  paragraph: { name: "zone", size: [240, 64], kind: () => text(null, { multiline: true }) },
  date: { name: "date", size: [100, 20], kind: () => text(FORMATS.date()) },
  amount: { name: "montant", size: [100, 20], kind: () => text(FORMATS.amount()) },
  email: { name: "email", size: [180, 20], kind: () => text(FORMATS.email()) },
  phone: { name: "telephone", size: [120, 20], kind: () => text(FORMATS.phone()) },
  checkbox: { name: "case", size: [14, 14], kind: () => ({ kind: "checkbox" }), export: "Oui" },
  radio: { name: "choix", size: [14, 14], kind: () => ({ kind: "radio", options: [] }), export: "Choix1" },
  dropdown: { name: "liste", size: [140, 20], kind: () => ({ kind: "choice", options: ["Option 1", "Option 2"], editable: false, list: false, multi: false }) },
  list: { name: "liste", size: [140, 60], kind: () => ({ kind: "choice", options: ["Option 1", "Option 2", "Option 3"], editable: false, list: true, multi: false }) },
  signature: { name: "signature", size: [180, 50], kind: () => ({ kind: "signature" }) },
};

export const DEFAULT_LOOK = {
  font_size: 0,
  align: "left",
  color: { r: 0, g: 0, b: 0 },
  border: { r: 0x80, g: 0x8a, b: 0x96 },
  background: null,
};

/** `prefix_1`, `prefix_2`… : le premier nom libre parmi `names`. */
export function uniqueName(prefix, names) {
  const taken = new Set(names);
  for (let n = 1; ; n += 1) {
    const name = `${prefix}_${n}`;
    if (!taken.has(name)) return name;
  }
}

/** Valeur libre d'un nouveau bouton radio : Choix1, Choix2… */
export function freeExport(exports) {
  for (let n = 1; ; n += 1) if (!exports.includes(`Choix${n}`)) return `Choix${n}`;
}

/** Boîte bornée à la page (points, origine en haut). */
export function clampBox(box, page) {
  const width = Math.min(Math.max(box.width, MIN_SIDE), page.width);
  const height = Math.min(Math.max(box.height, MIN_SIDE), page.height);
  return {
    x: Math.min(Math.max(box.x, 0), page.width - width),
    y: Math.min(Math.max(box.y, 0), page.height - height),
    width,
    height,
  };
}

/** Boîte tracée entre deux points ; `null` si trop petite (c'est un clic). */
export function boxBetween(start, end, page) {
  const width = Math.abs(end.x - start.x);
  const height = Math.abs(end.y - start.y);
  if (width < 6 && height < 6) return null;
  return clampBox({ x: Math.min(start.x, end.x), y: Math.min(start.y, end.y), width: Math.max(width, MIN_SIDE * 2), height: Math.max(height, MIN_SIDE * 2) }, page);
}

/** Boîte d'un simple clic : la taille de l'outil, coin haut gauche sous la souris. */
export function boxAt(point, tool, page) {
  const [width, height] = TOOLS[tool].size;
  return clampBox({ x: point.x, y: point.y - height / 2, width, height }, page);
}

/**
 * Nouveau champ de l'outil `tool` dans `box` (page `page`), nommé
 * librement parmi `names` ; un bouton radio rejoint `group` s'il est donné.
 */
export function newField(tool, box, page, names, group = null) {
  const preset = TOOLS[tool];
  const field = {
    page,
    top_left: { x: box.x, y: box.y },
    width: box.width,
    height: box.height,
    name: group?.name ?? uniqueName(preset.name, names),
    // `field_kind` : dans un élément, `kind` désigne déjà l'élément (« field »).
    field_kind: preset.kind(),
    tooltip: "",
    required: false,
    read_only: false,
    look: { ...DEFAULT_LOOK, border: tool === "signature" ? { r: 0x8a, g: 0x1c, b: 0x2b } : DEFAULT_LOOK.border },
    value: null,
    export: preset.export ?? "",
    origin: null,
    deleted: false,
  };
  if (group) field.export = freeExport(group.exports);
  return field;
}

/** Valeur de la liste « Format » pour le format d'un champ. */
export function formatChoice(format) {
  if (!format) return "none";
  if (format.kind === "number") return format.currency ? "amount" : "number";
  if (format.kind === "mask") {
    if (format.mask === "99 99 99 99 99") return "phone";
    if (format.mask === "99999") return "zip";
    return "mask";
  }
  return format.kind;
}

/**
 * Format choisi dans la liste ; garde les réglages du format actuel quand
 * on reste dans le même genre (motif de date, décimales).
 */
export function formatFor(choice, current) {
  const fresh = FORMATS[choice]?.() ?? null;
  if (!fresh || !current || current.kind !== fresh.kind) return fresh;
  if (choice === "amount" && !current.currency) return fresh;
  if (choice === "number" && current.currency) return fresh;
  if (fresh.kind === "mask" && choice !== "mask") return fresh;
  return { ...current };
}

/**
 * Réglages d'un champ (FieldView) en modèle pour set_form_field ; `patch`
 * change certains réglages.
 */
export function templateOf(field, patch = {}) {
  const widget = field.widgets[0];
  return {
    page: widget?.page ?? 0,
    top_left: { x: widget?.bounds.x ?? 0, y: widget?.bounds.y ?? 0 },
    width: Math.max(widget?.bounds.width ?? 10, MIN_SIDE),
    height: Math.max(widget?.bounds.height ?? 10, MIN_SIDE),
    name: field.name,
    field_kind: field.kind,
    tooltip: field.tooltip ?? "",
    required: field.required,
    read_only: field.read_only,
    look: field.look ?? DEFAULT_LOOK,
    value: field.created ? field.value : null,
    export: widget?.export ?? "",
    origin: null,
    deleted: false,
    ...patch,
  };
}

/** Options d'une liste écrites une par ligne → tableau sans doublon ni vide. */
export function parseOptions(textValue) {
  const seen = new Set();
  return textValue
    .split("\n")
    .map((o) => o.trim())
    .filter((o) => o && !seen.has(o) && seen.add(o));
}

const MARKS = {
  comma_dot: [",", "."],
  dot: [null, "."],
  dot_comma: [".", ","],
  comma: [null, ","],
  apostrophe_dot: ["'", "."],
};

/**
 * Valeur rangée (« 1234.5 ») telle qu'un lecteur l'affiche (« 1234,50 € »),
 * comme FieldFormat::display de pde-core ; autre format : inchangée.
 */
export function displayValue(format, value) {
  if (!format || !value || (format.kind !== "number" && format.kind !== "percent")) return value ?? "";
  let number = Number(String(value).trim());
  if (!Number.isFinite(number)) return value;
  if (format.kind === "percent") number *= 100;
  const fixed = number.toFixed(format.decimals ?? 0);
  const [sign, digits] = fixed.startsWith("-") && Number(fixed) !== 0 ? ["-", fixed.slice(1)] : ["", fixed.replace(/^-/, "")];
  const [int, frac] = digits.split(".");
  const [group, mark] = MARKS[format.separator ?? "comma"] ?? MARKS.comma;
  const grouped = group ? int.replace(/\B(?=(\d{3})+(?!\d))/g, group) : int;
  const shown = `${sign}${grouped}${frac ? mark + frac : ""}`;
  if (format.kind === "percent") return `${shown}%`;
  return format.currency_before ? `${format.currency}${shown}` : `${shown}${format.currency ?? ""}`;
}

/** Emplacements d'une page : [{ field, widget }] (champs de signature compris). */
export function boxesOn(fields, page) {
  return fields.flatMap((field) => field.widgets.filter((w) => w.page === page).map((widget) => ({ field, widget })));
}

/** Libellé court d'un genre de champ, clé de traduction. */
export function kindKey(field) {
  const kind = field.kind.kind;
  if (kind === "text") {
    const choice = formatChoice(field.kind.format);
    if (choice !== "none") return `formDesign.format.${choice}`;
    return field.kind.multiline ? "formDesign.tool.paragraph" : "formDesign.tool.text";
  }
  if (kind === "choice") return field.kind.list ? "formDesign.tool.list" : "formDesign.tool.dropdown";
  return `formDesign.tool.${kind}`;
}

/** Empreinte des éléments de conception d'un état d'édition : change quand un champ est créé, réglé, déplacé ou supprimé. */
export function fieldSignature(view) {
  return JSON.stringify((view?.elements ?? []).filter((e) => e.element.kind === "field").map((e) => [e.id, e.element]));
}

/** Cases au plus dans un champ en cases de `width` points (1 à 100). */
export function maxCombCells(width) {
  return Math.min(Math.max(Math.floor(width / MIN_COMB_CELL), 1), 100);
}

/**
 * Longueur d'un champ texte selon « Une case par caractère » : sans cases,
 * celle saisie ; en cases, celle saisie si elle tient dans la largeur,
 * sinon des cases à peu près carrées (bogue #57 : 100 cases dans 220 pt
 * empilaient les caractères).
 */
export function combLength(comb, maxLen, width, height) {
  if (!comb) return maxLen;
  const max = maxCombCells(width);
  if (maxLen && maxLen <= max) return maxLen;
  const square = Math.floor(width / Math.max(MIN_COMB_CELL, height * 0.75));
  return Math.min(Math.max(square, 1), max);
}
