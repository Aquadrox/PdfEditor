// Petits utilitaires DOM. Le texte passe toujours par textContent : un nom
// de fichier ou un titre de PDF ne peut pas injecter de HTML.

export function byId(id) {
  const el = document.getElementById(id);
  if (!el) throw new Error(`élément #${id} absent de index.html`);
  return el;
}

/** Crée un élément : el("button", { class: "btn", onclick }, "texte", enfant…). */
export function el(tag, attributes = {}, ...children) {
  const node = document.createElement(tag);
  for (const [name, value] of Object.entries(attributes)) {
    if (value === undefined || value === null || value === false) continue;
    if (name.startsWith("on")) node.addEventListener(name.slice(2), value);
    else if (name === "class") node.className = value;
    else node.setAttribute(name, value === true ? "" : String(value));
  }
  for (const child of children) {
    node.append(typeof child === "string" ? document.createTextNode(child) : child);
  }
  return node;
}

/** Nom du fichier sans le dossier (séparateurs Windows et Unix). */
export function fileName(path) {
  return path.split(/[\\/]/).pop() || path;
}
