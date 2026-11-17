// Calculs des annotations dessinées (E-04), sans DOM : création d'une
// forme, d'un trait à main levée ou d'un marquage de texte, cadre englobant,
// déplacement, redimensionnement, extrémités d'un trait. Module pur, testé
// par annotation-state.test.js. Coordonnées : points de la page affichée,
// origine en haut à gauche.

/** Couleurs proposées pour les annotations (couleurs du document). */
export const ANNOTATION_COLORS = [
  { name: "yellow", hex: "#ffd54a" },
  { name: "red", hex: "#c62828" },
  { name: "blue", hex: "#2747b8" },
  { name: "green", hex: "#1e7344" },
  { name: "ink", hex: "#1c2430" },
];

/** Réglages de départ de chaque outil. */
export const TOOL_DEFAULTS = {
  markup: { mark: "highlight", color: { r: 0xff, g: 0xd5, b: 0x4a } },
  ink: { color: { r: 0xc6, g: 0x28, b: 0x28 }, width: 2 },
  shape: { shape: "rectangle", color: { r: 0xc6, g: 0x28, b: 0x28 }, width: 2, filled: false },
};

export const ANNOTATION_KINDS = ["shape", "ink", "markup"];
/** En dessous (points), un glisser est un clic : pas de forme. */
export const MIN_SHAPE_SIZE = 3;
/** Distance minimale entre deux points gardés d'un trait à main levée. */
const MIN_STEP = 1.5;

export const isAnnotation = (element) => ANNOTATION_KINDS.includes(element?.kind);

function clamp(value, min, max) {
  return Math.min(Math.max(value, min), Math.max(min, max));
}

function clampPoint(p, page) {
  return { x: clamp(p.x, 0, page.width), y: clamp(p.y, 0, page.height) };
}

/**
 * Fin d'un glisser avec Maj : carré / cercle pour les cadres, angle multiple
 * de 45° pour les traits.
 */
export function constrained(shape, start, end) {
  const dx = end.x - start.x;
  const dy = end.y - start.y;
  if (shape === "rectangle" || shape === "ellipse") {
    const size = Math.max(Math.abs(dx), Math.abs(dy));
    return { x: start.x + Math.sign(dx || 1) * size, y: start.y + Math.sign(dy || 1) * size };
  }
  const length = Math.hypot(dx, dy);
  const angle = Math.round(Math.atan2(dy, dx) / (Math.PI / 4)) * (Math.PI / 4);
  return { x: start.x + length * Math.cos(angle), y: start.y + length * Math.sin(angle) };
}

/** Forme tracée de `start` à `end` ; null si le glisser est trop court. */
export function newShape(pageIndex, start, end, style, page) {
  const a = clampPoint(start, page);
  const b = clampPoint(end, page);
  if (Math.hypot(b.x - a.x, b.y - a.y) < MIN_SHAPE_SIZE) return null;
  return { kind: "shape", page: pageIndex, shape: style.shape, start: a, end: b, color: { ...style.color }, width: style.width, filled: style.filled };
}

/** Ajoute un point au tracé en cours s'il s'éloigne assez du précédent. */
export function withPoint(points, p, page) {
  const point = clampPoint(p, page);
  const last = points[points.length - 1];
  if (last && Math.hypot(point.x - last.x, point.y - last.y) < MIN_STEP) return points;
  return [...points, point];
}

/** Trait à main levée ; null pour un simple clic (rien de visible à garder). */
export function newInk(pageIndex, points, style) {
  if (points.length < 2) return null;
  return { kind: "ink", page: pageIndex, points, color: { ...style.color }, width: style.width };
}

/**
 * Marquage de texte à partir des rectangles d'une sélection (points de la
 * page) : les morceaux d'une même ligne sont réunis.
 */
export function newMarkup(pageIndex, rects, style) {
  const lines = [];
  const sorted = rects
    .filter((r) => r.width > 0.5 && r.height > 0.5)
    .sort((a, b) => a.y - b.y || a.x - b.x);
  for (const r of sorted) {
    const line = lines.find((l) => Math.abs(l.y + l.height / 2 - (r.y + r.height / 2)) < Math.min(l.height, r.height) / 2 && r.x <= l.x + l.width + r.height);
    if (line) {
      const right = Math.max(line.x + line.width, r.x + r.width);
      const bottom = Math.max(line.y + line.height, r.y + r.height);
      line.x = Math.min(line.x, r.x);
      line.y = Math.min(line.y, r.y);
      line.width = right - line.x;
      line.height = bottom - line.y;
    } else {
      lines.push({ ...r });
    }
  }
  if (lines.length === 0) return null;
  return {
    kind: "markup",
    page: pageIndex,
    mark: style.mark,
    areas: lines.map((l) => ({ top_left: { x: l.x, y: l.y }, width: l.width, height: l.height })),
    color: { ...style.color },
  };
}

/** Points qui définissent l'annotation. */
function pointsOf(element) {
  if (element.kind === "shape") return [element.start, element.end];
  if (element.kind === "ink") return element.points;
  return element.areas.flatMap((a) => [a.top_left, { x: a.top_left.x + a.width, y: a.top_left.y + a.height }]);
}

/** Cadre des points (sans l'épaisseur du trait) : { x, y, width, height }. */
export function bboxOf(element) {
  const points = pointsOf(element);
  const xs = points.map((p) => p.x);
  const ys = points.map((p) => p.y);
  const x = Math.min(...xs);
  const y = Math.min(...ys);
  return { x, y, width: Math.max(...xs) - x, height: Math.max(...ys) - y };
}

function mapPoints(element, f) {
  if (element.kind === "shape") return { ...element, start: f(element.start), end: f(element.end) };
  if (element.kind === "ink") return { ...element, points: element.points.map(f) };
  return { ...element, areas: element.areas.map((a) => ({ ...a, top_left: f(a.top_left) })) };
}

/** Annotation déplacée de (dx, dy), sans sortir de la page. */
export function translated(element, dx, dy, page) {
  const box = bboxOf(element);
  const mx = clamp(box.x + dx, 0, page.width - box.width) - box.x;
  const my = clamp(box.y + dy, 0, page.height - box.height) - box.y;
  return mapPoints(element, (p) => ({ x: p.x + mx, y: p.y + my }));
}

/** Annotation étirée de son cadre `from` vers le cadre `to`. */
export function reframed(element, from, to) {
  const sx = from.width > 0 ? to.width / from.width : 1;
  const sy = from.height > 0 ? to.height / from.height : 1;
  return mapPoints(element, (p) => ({ x: to.x + (p.x - from.x) * sx, y: to.y + (p.y - from.y) * sy }));
}

/** Poignées d'une annotation : extrémités d'un trait, coins sinon, aucune pour un marquage. */
export function handlesOf(element) {
  if (element.kind === "markup") return [];
  if (element.kind === "shape" && (element.shape === "line" || element.shape === "arrow")) return ["start", "end"];
  return ["nw", "ne", "sw", "se"];
}

/**
 * Poignée `edge` tirée de (dx, dy) : extrémité d'un trait, ou coin du cadre
 * (le coin opposé ne bouge pas).
 */
export function annotationResized(element, edge, dx, dy, page) {
  if (edge === "start" || edge === "end") {
    const p = element[edge];
    return { ...element, [edge]: clampPoint({ x: p.x + dx, y: p.y + dy }, page) };
  }
  const box = bboxOf(element);
  const west = edge.includes("w");
  const north = edge.includes("n");
  const left = west ? clamp(box.x + dx, 0, box.x + box.width - 1) : box.x;
  const right = west ? box.x + box.width : clamp(box.x + box.width + dx, box.x + 1, page.width);
  const top = north ? clamp(box.y + dy, 0, box.y + box.height - 1) : box.y;
  const bottom = north ? box.y + box.height : clamp(box.y + box.height + dy, box.y + 1, page.height);
  return reframed(element, box, { x: left, y: top, width: right - left, height: bottom - top });
}

/** Position (x, y) ou taille (width, height) saisie, en points ; null si impossible. */
export function annotationWithGeometry(element, field, value, page) {
  if (element.kind === "markup" && (field === "width" || field === "height")) return null;
  const box = bboxOf(element);
  if (field === "x") return translated(element, value - box.x, 0, page);
  if (field === "y") return translated(element, 0, value - box.y, page);
  const to = { ...box, [field]: clamp(value, 1, field === "width" ? page.width - box.x : page.height - box.y) };
  return reframed(element, box, to);
}

/** Style changé : couleur, épaisseur, remplissage, forme ou marquage. */
export function withAnnotationStyle(element, changes) {
  const allowed = { shape: ["color", "width", "filled", "shape"], ink: ["color", "width"], markup: ["color", "mark"] }[element.kind];
  const kept = Object.fromEntries(Object.entries(changes).filter(([key]) => allowed.includes(key)));
  return { ...element, ...kept };
}
