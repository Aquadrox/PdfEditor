// Menu déroulant attaché à un bouton : ouverture au clic, fermeture par
// Échap, clic à l'extérieur ou choix d'une entrée ; flèches pour naviguer.

export function attachMenu(button, menu) {
  const items = () => [...menu.querySelectorAll("button:not(:disabled)")];

  function close() {
    menu.hidden = true;
    button.setAttribute("aria-expanded", "false");
  }

  function open() {
    menu.hidden = false;
    button.setAttribute("aria-expanded", "true");
    items()[0]?.focus();
  }

  button.setAttribute("aria-haspopup", "menu");
  button.setAttribute("aria-expanded", "false");
  button.addEventListener("click", () => (menu.hidden ? open() : close()));
  menu.addEventListener("click", (event) => {
    if (event.target.closest("button")) close();
  });
  menu.addEventListener("keydown", (event) => {
    const list = items();
    const index = list.indexOf(document.activeElement);
    if (event.key === "Escape") {
      close();
      button.focus();
    } else if (event.key === "ArrowDown") {
      list[(index + 1) % list.length]?.focus();
      event.preventDefault();
    } else if (event.key === "ArrowUp") {
      list[(index - 1 + list.length) % list.length]?.focus();
      event.preventDefault();
    }
  });
  document.addEventListener("pointerdown", (event) => {
    if (!menu.hidden && !menu.contains(event.target) && !button.contains(event.target)) close();
  });
  return { close };
}
