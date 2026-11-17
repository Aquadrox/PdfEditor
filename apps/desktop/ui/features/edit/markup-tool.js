// Outil Surligner : glisser sur le texte d'une page le sélectionne (aperçu
// par la sélection du navigateur) ; au relâchement, la sélection devient un
// marquage (surligné, souligné ou barré).
//
// La sélection n'est pas laissée au navigateur : sur un texte posé en
// positions absolues, il l'étend parfois jusqu'au bas de la page dès que
// la souris quitte une ligne. Début et fin sont pris au caractère sous la
// souris, ou au bout de la ligne la plus proche ; la hauteur des bandes est
// celle du texte dans le PDF (boîte du segment), pas celle de la police
// d'affichage.

import { newMarkup } from "./annotation-state.js";

/**
 * Place du texte d'un segment à l'écran. Pas celle du `<span>` : sa boîte,
 * déjà à la largeur du PDF, est étirée une seconde fois avec le texte.
 */
function textRect(span) {
  const range = document.createRange();
  range.selectNodeContents(span);
  return range.getBoundingClientRect();
}

/** Segment de texte le plus proche du point : d'abord sur la même ligne. */
function nearestSpan(spans, x, y) {
  let best = null;
  let bestScore = Infinity;
  for (const span of spans) {
    const r = textRect(span);
    // Dans la ligne : 0 ; sinon distance au bord le plus proche. Une
    // tolérance plus large prendrait la ligne suivante (interligne serré).
    const dy = y < r.top ? r.top - y : y > r.bottom ? y - r.bottom : 0;
    const dx = x < r.left ? r.left - x : x > r.right ? x - r.right : 0;
    // Même ligne : la distance verticale l'emporte toujours.
    const score = dy * 1000 + dx;
    if (score < bestScore) {
      best = span;
      bestScore = score;
    }
  }
  return best;
}

/** Position (segment, caractère) sous (x, y), ramenée au segment le plus proche. */
export function caretAt(layer, x, y) {
  const spans = [...layer.querySelectorAll("span")];
  const span = nearestSpan(spans, x, y);
  if (!span?.firstChild) return null;
  const text = span.firstChild;
  const r = textRect(span);
  if (x <= r.left) return { span, node: text, offset: 0 };
  if (x >= r.right) return { span, node: text, offset: text.length };
  const caret = document.caretRangeFromPoint?.(x, (r.top + r.bottom) / 2);
  const offset = caret?.startContainer === text ? caret.startOffset : Math.round(((x - r.left) / r.width) * text.length);
  return { span, node: text, offset };
}

function ordered(a, b) {
  const position = a.node.compareDocumentPosition(b.node);
  const before = a.node === b.node ? a.offset <= b.offset : Boolean(position & Node.DOCUMENT_POSITION_FOLLOWING);
  return before ? [a, b] : [b, a];
}

/** Rectangles (points de la page) du texte entre deux positions. */
export function selectedRects(layer, from, to, page) {
  const [start, end] = ordered(from, to);
  const spans = [...layer.querySelectorAll("span")];
  const first = spans.indexOf(start.span);
  const last = spans.indexOf(end.span);
  const frame = layer.getBoundingClientRect();
  const ratio = page.width / frame.width;
  const rects = [];
  for (const span of spans.slice(first, last + 1)) {
    const text = span.firstChild;
    if (!text) continue;
    const a = span === start.span ? start.offset : 0;
    const b = span === end.span ? end.offset : text.length;
    if (b <= a) continue;
    const range = document.createRange();
    range.setStart(text, a);
    range.setEnd(text, b);
    const r = range.getBoundingClientRect();
    // Début et fin de segment : position exacte du PDF (la police
    // d'affichage peut différer de quelques pour cent) ; ailleurs, l'écran.
    const pdfLeft = (Number.parseFloat(span.style.left) / 100) * page.width;
    const pdfRight = pdfLeft + (Number.parseFloat(span.dataset.width) / 100) * page.width;
    const left = a === 0 ? pdfLeft : (r.left - frame.left) * ratio;
    const right = b === text.length && span.dataset.width ? pdfRight : (r.right - frame.left) * ratio;
    rects.push({
      x: left,
      width: right - left,
      // Hauteur du segment dans le PDF (style posé en % de la page).
      y: (Number.parseFloat(span.style.top) / 100) * page.height,
      height: (Number.parseFloat(span.style.height) / 100) * page.height,
    });
  }
  return rects;
}

function showSelection(from, to) {
  const [start, end] = ordered(from, to);
  const range = document.createRange();
  range.setStart(start.node, start.offset);
  range.setEnd(end.node, end.offset);
  const selection = window.getSelection();
  selection.removeAllRanges();
  selection.addRange(range);
}

/**
 * Suit un glisser sur la couche de texte `layer` d'une page ;
 * `done(rectangles)` au relâchement (points de la page). Faux si le
 * glisser ne part d'aucun texte.
 */
export function trackTextSelection(event, layer, page, done) {
  const from = caretAt(layer, event.clientX, event.clientY);
  if (!from) return false;
  let to = from;
  const move = (e) => {
    to = caretAt(layer, e.clientX, e.clientY) ?? to;
    showSelection(from, to);
  };
  const up = () => {
    window.removeEventListener("pointermove", move);
    window.removeEventListener("pointerup", up);
    window.getSelection()?.removeAllRanges();
    done(selectedRects(layer, from, to, page));
  };
  window.addEventListener("pointermove", move);
  window.addEventListener("pointerup", up);
  return true;
}

/**
 * Surveille la table : un glisser sur le texte d'une page, outil Surligner
 * actif (`enabled()`), donne `onMarkup(élément)` au relâchement.
 */
export function attachMarkupTool(viewer, enabled, style, onMarkup) {
  viewer.desk.addEventListener("pointerdown", (event) => {
    const layer = event.target.closest?.(".text-layer");
    if (!enabled() || !layer || event.button !== 0) return;
    event.preventDefault();
    const index = viewer.frames.indexOf(layer.parentElement);
    const page = viewer.doc?.pages[index];
    if (!page) return;
    trackTextSelection(event, layer, page, (rects) => {
      const markup = newMarkup(index, rects, style());
      if (markup) onMarkup(markup);
    });
  });
}
