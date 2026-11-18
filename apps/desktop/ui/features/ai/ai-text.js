// Texte des réponses de l'IA (module pur, testé par ai-text.test.js) :
// Markdown réduit (titres, listes, gras) et citations de pages [p. 3],
// vérifiées contre le nombre de pages avant de devenir des liens. Le rendu
// se fait toujours en textContent (jamais de HTML venu du modèle).

/** Pages choisies (« 2-5, 8 », numéros à partir de 1) → rangs à partir de 0 ; null si invalide. */
export function parsePages(text, pageCount) {
  const pages = new Set();
  const parts = text.split(/[,;]/).map((p) => p.trim()).filter(Boolean);
  if (parts.length === 0) return null;
  for (const part of parts) {
    const match = /^(\d+)\s*(?:[-–]\s*(\d+))?$/.exec(part);
    if (!match) return null;
    const first = Number(match[1]);
    const last = Number(match[2] ?? match[1]);
    if (first < 1 || last < first || last > pageCount) return null;
    for (let page = first; page <= last; page++) pages.add(page - 1);
  }
  return [...pages].sort((a, b) => a - b);
}

const CITATION = /\[(?:p\.|pp\.|pages?)\s*([\d\s,–\-et]+)\]/gi;

/** Pages d'une citation (« 3-4, 7 » → [3, 4, 7]), numéros à partir de 1. */
function citedPages(list) {
  const pages = [];
  for (const part of list.split(/,|\bet\b/)) {
    const match = /(\d+)\s*(?:[-–]\s*(\d+))?/.exec(part);
    if (!match) continue;
    const first = Number(match[1]);
    const last = Math.min(Number(match[2] ?? match[1]), first + 50);
    for (let page = first; page <= last; page++) pages.push(page);
  }
  return pages;
}

/**
 * Morceaux d'une ligne : texte, gras, citations. Une citation porte ses
 * pages (à partir de 1) et `valid` : toutes existent dans le document.
 */
export function inline(text, pageCount) {
  const parts = [];
  let last = 0;
  for (const match of text.matchAll(CITATION)) {
    if (match.index > last) parts.push(...bold(text.slice(last, match.index)));
    const pages = citedPages(match[1]);
    const valid = pages.length > 0 && pages.every((p) => p >= 1 && p <= pageCount);
    parts.push({ kind: "cite", text: match[0], pages, valid });
    last = match.index + match[0].length;
  }
  if (last < text.length) parts.push(...bold(text.slice(last)));
  return parts;
}

function bold(text) {
  const parts = [];
  const pieces = text.split(/\*\*(.+?)\*\*/);
  pieces.forEach((piece, index) => {
    if (piece) parts.push({ kind: index % 2 ? "bold" : "text", text: piece });
  });
  return parts;
}

/** Blocs d'une réponse : titres, éléments de liste, paragraphes. */
export function blocks(text, pageCount) {
  const out = [];
  for (const raw of text.split("\n")) {
    const line = raw.trimEnd();
    if (!line.trim()) continue;
    const heading = /^#{1,4}\s+(.*)$/.exec(line);
    const item = /^\s*(?:[-*•]|\d+[.)])\s+(.*)$/.exec(line);
    if (heading) out.push({ kind: "heading", parts: inline(heading[1], pageCount) });
    else if (item) out.push({ kind: "item", parts: inline(item[1], pageCount) });
    else out.push({ kind: "paragraph", parts: inline(line, pageCount) });
  }
  return out;
}
