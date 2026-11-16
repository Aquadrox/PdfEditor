// Brouillon des réglages : lecture et écriture par chemin (« general.theme »),
// nombre de valeurs modifiées. Module pur, testé.

export function getPath(object, path) {
  return path.split(".").reduce((value, key) => value?.[key], object);
}

/** Copie de `object` avec `path` remplacé par `value`. */
export function setPath(object, path, value) {
  const copy = structuredClone(object);
  const keys = path.split(".");
  const last = keys.pop();
  keys.reduce((node, key) => (node[key] ??= {}), copy)[last] = value;
  return copy;
}

/** Nombre de valeurs (feuilles) différentes entre deux réglages. */
export function countChanges(a, b) {
  if (typeof a !== "object" || a === null || typeof b !== "object" || b === null) {
    return Object.is(a, b) ? 0 : 1;
  }
  const keys = new Set([...Object.keys(a), ...Object.keys(b)]);
  let count = 0;
  for (const key of keys) count += countChanges(a[key], b[key]);
  return count;
}
