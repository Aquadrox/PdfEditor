// Seul module qui parle à Tauri : commandes, événements, adresses des pages.

function tauri() {
  return window.__TAURI__;
}

/** Appelle une commande Rust ; l'erreur rejetée est { kind, message }. */
export function invoke(command, args = {}) {
  return tauri().core.invoke(command, args);
}

export function listen(event, handler) {
  return tauri().event.listen(event, handler);
}

export function setWindowTitle(title) {
  return tauri().window.getCurrentWindow().setTitle(title);
}

/** Le WebView de Windows écrit les protocoles maison en http://nom.localhost. */
function protocolBase() {
  return navigator.userAgent.includes("Windows") ? "http://pde.localhost/" : "pde://localhost/";
}

/** Image PNG d'une page (protocole pde://, servi par apps/desktop/src/protocol.rs). */
export function pageUrl(doc, page, scale) {
  return `${protocolBase()}page/${doc}/${page}?scale=${scale}`;
}
