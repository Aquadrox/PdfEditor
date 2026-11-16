// Notification non bloquante, en bas de la fenêtre.

let timer = 0;

/** Affiche `message` quelques secondes ; `kind` : "info" ou "error". */
export function toast(message, kind = "info") {
  const box = document.getElementById("toast");
  if (!box) return;
  box.textContent = message;
  box.className = kind === "error" ? "toast error" : "toast";
  box.hidden = false;
  clearTimeout(timer);
  timer = setTimeout(() => {
    box.hidden = true;
  }, kind === "error" ? 8000 : 4000);
}
