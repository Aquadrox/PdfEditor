// Règles de la signature vues de l'interface (module pur, testé par
// sign-state.test.js) : état du sceau, verrouillage, textes du panneau
// Signatures, lignes de l'apparence et cadre tracé à la souris.
// Les signatures viennent de pde-app (signing/view.rs).

/** Plus petit cadre de signature accepté, en points. */
export const MIN_FRAME = 24;

/**
 * État du document pour le sceau de la barre du haut.
 * tone : "none" (pas de signature), "valid", "warn" (modifié depuis, ou
 * champ vide), "invalid" ; locked : plus aucune modification permise.
 */
export function summary(signatures) {
  const signed = signatures.filter((s) => s.state !== "empty");
  const valid = signed.filter((s) => s.state === "valid");
  const empty = signatures.length - signed.length;
  const lock = documentLock(signatures);
  let tone = "none";
  if (signed.length > 0) {
    if (signed.some((s) => s.state === "invalid")) tone = "invalid";
    else if (valid.length < signed.length || empty > 0 || valid.some((s) => s.modified_after && !s.lock)) tone = "warn";
    else tone = "valid";
  } else if (empty > 0) {
    tone = "warn";
  }
  return { signed: signed.length, valid: valid.length, empty, total: signatures.length, tone, lock };
}

/**
 * Verrouillage posé par une signature de certification valide : "all"
 * (aucune modification), "forms" (formulaire et signatures), ou null.
 */
export function documentLock(signatures) {
  const certification = signatures.find((s) => s.state === "valid" && s.lock);
  return certification ? certification.lock : null;
}

/** Peut-on encore signer ? Verrouiller exige d'être la première signature. */
export function signingRules(signatures) {
  const lock = documentLock(signatures);
  const signed = signatures.some((s) => s.state !== "empty");
  return { canSign: lock !== "all", canLock: !signed };
}

/** Clé et paramètres du libellé du sceau (t()). */
export function sealLabel(info) {
  if (info.lock === "all") return ["seal.locked", {}];
  if (info.tone === "invalid") return ["seal.invalid", {}];
  if (info.signed === 0) return ["seal.toSign", { count: info.empty }];
  if (info.empty > 0) return ["seal.partial", { signed: info.valid, total: info.total }];
  return [info.valid > 1 ? "seal.valid.many" : "seal.valid.one", { count: info.valid }];
}

/** Date et heure en français (« 18/11/2026 à 14:32 »). */
export function formatTime(seconds) {
  const date = new Date(seconds * 1000);
  const day = date.toLocaleDateString("fr-FR", { day: "2-digit", month: "2-digit", year: "numeric" });
  const time = date.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  return { day, time };
}

/**
 * Lignes affichées sous une signature du panneau : [clé, paramètres][].
 */
export function details(signature) {
  const lines = [];
  const page = signature.page === null || signature.page === undefined ? null : signature.page + 1;
  if (signature.state === "empty") {
    lines.push(page ? ["sig.emptyOnPage", { page }] : ["sig.empty", {}]);
    return lines;
  }
  if (signature.time) {
    const { day, time } = formatTime(signature.time);
    lines.push(page ? ["sig.signedOnPage", { day, time, page }] : ["sig.signedAt", { day, time }]);
  }
  if (signature.state === "valid") {
    lines.push(signature.modified_after ? ["sig.modifiedAfter", {}] : ["sig.unchanged", {}]);
  } else if (signature.problem) {
    lines.push(["sig.problem", { problem: signature.problem }]);
  }
  if (signature.reason) lines.push(["sig.reason", { reason: signature.reason }]);
  if (signature.location) lines.push(["sig.location", { location: signature.location }]);
  if (signature.lock) lines.push([`sig.lock.${signature.lock}`, {}]);
  if (signature.state !== "invalid") {
    lines.push(signature.self_signed ? ["sig.selfSigned", {}] : ["sig.issuer", { issuer: signature.issuer ?? "" }]);
  }
  return lines;
}

/** Libellé court de l'état d'une signature. */
export function stateLabel(signature) {
  return `sig.state.${signature.state}`;
}

/**
 * Lignes écrites dans le cadre d'une signature visible (rédigées ici, en
 * français, à l'heure locale du signataire).
 */
export function appearanceLines(identity, now = new Date()) {
  const { day, time } = formatTime(now.getTime() / 1000);
  const lines = [`Signé numériquement par ${identity.name}`];
  if (identity.organization) lines.push(identity.organization);
  lines.push(`le ${day} à ${time}`);
  return lines;
}

/** Cadre tracé entre deux points (points de la page, origine en haut), borné à la page ; null s'il est trop petit. */
export function frameBetween(start, end, page) {
  const clamp = (v, max) => Math.min(Math.max(v, 0), max);
  const x0 = clamp(Math.min(start.x, end.x), page.width);
  const y0 = clamp(Math.min(start.y, end.y), page.height);
  const x1 = clamp(Math.max(start.x, end.x), page.width);
  const y1 = clamp(Math.max(start.y, end.y), page.height);
  if (x1 - x0 < MIN_FRAME || y1 - y0 < MIN_FRAME / 2) return null;
  return { x: x0, y: y0, width: x1 - x0, height: y1 - y0 };
}

/** Cadre par défaut d'un simple clic : 180 × 54 pt, posé sous la souris, dans la page. */
export function frameAt(point, page) {
  const width = Math.min(180, page.width);
  const height = Math.min(54, page.height);
  return {
    x: Math.min(Math.max(point.x - width / 2, 0), page.width - width),
    y: Math.min(Math.max(point.y - height / 2, 0), page.height - height),
    width,
    height,
  };
}

/** Libellé d'une identité dans les listes. */
export function identityLabel(identity) {
  const kind = identity.self_signed ? "personnel" : `émis par ${identity.issuer}`;
  return `${identity.name}, certificat ${kind}`;
}
