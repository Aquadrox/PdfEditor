// Seul module qui parle à Tauri : commandes, événements, adresses des pages.

function tauri() {
  return window.__TAURI__;
}

/** Appelle une commande Rust ; l'erreur rejetée est { kind, message }. */
export function invoke(command, args = {}) {
  return tauri().core.invoke(command, args);
}

/** Commande dont le corps est un contenu brut (octets), sans JSON. */
export function invokeBytes(command, bytes, headers = {}) {
  return tauri().core.invoke(command, bytes, { headers });
}

export function listen(event, handler) {
  return tauri().event.listen(event, handler);
}

export function setWindowTitle(title) {
  return tauri().window.getCurrentWindow().setTitle(title);
}

/**
 * Adresse du protocole maison selon le système : les WebView de Windows et
 * d'Android l'écrivent http://nom.localhost (sur Android, pde://localhost
 * n'aboutit pas : chaque page affichait « Cette page n'a pas pu être
 * affichée »).
 */
export function protocolBaseFor(userAgent) {
  return /Windows|Android/.test(userAgent) ? "http://pde.localhost/" : "pde://localhost/";
}

function protocolBase() {
  return protocolBaseFor(navigator.userAgent);
}

/**
 * Image PNG d'une page (protocole pde://, servi par apps/desktop/src/protocol.rs).
 * `revision` : version du contenu de la page ; une page modifiée a une
 * nouvelle adresse, le WebView ne ressert pas l'ancienne image.
 */
export function pageUrl(doc, page, scale, revision = 0) {
  const rev = revision ? `&rev=${revision}` : "";
  return `${protocolBase()}page/${doc}/${page}?scale=${scale}${rev}`;
}
