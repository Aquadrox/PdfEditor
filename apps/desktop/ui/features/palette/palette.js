// Palette de commandes (Ctrl+K) et « Comment faire ? » (F1) : toutes les
// actions du catalogue par leur nom, avec leur raccourci. Dans la palette,
// Entrée exécute et Maj+Entrée montre l'outil ; dans « Comment faire ? »,
// Entrée montre l'outil et chaque action est expliquée.

import { el, byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { shortcutLabel } from "../../lib/shortcuts.js";
import { toast } from "../../components/toast.js";
import { createRunner } from "./palette-runner.js";
import { searchActions } from "./palette-search.js";

export class Palette {
  /**
   * @param {Parameters<typeof createRunner>[0]} app
   * @param {() => Record<string, string>} shortcuts raccourcis réglés
   */
  constructor(app, shortcuts) {
    this.runner = createRunner(app);
    this.shortcuts = shortcuts;
    this.actions = [];
    this.results = [];
    this.active = 0;
    this.help = false;
    this.dialog = byId("palette");
    this.input = byId("palette-input");
    this.list = byId("palette-list");
    this.input.addEventListener("input", () => this.render());
    this.input.addEventListener("keydown", (event) => this.onKey(event));
    this.list.addEventListener("pointerdown", (event) => event.preventDefault());
    this.dialog.addEventListener("close", () => this.list.replaceChildren());
    // Clic sur le fond : fermeture, comme Échap.
    this.dialog.addEventListener("click", (event) => {
      if (event.target === this.dialog) this.dialog.close();
    });
  }

  /** Charge le catalogue ; la palette reste vide (et le dit) en cas d'échec. */
  async load() {
    this.actions = await invoke("action_catalog").catch(() => []);
  }

  /**
   * @param {"run" | "help"} kind
   * @param {string} query recherche de départ (« Comment faire ? » depuis l'assistant)
   */
  open(kind = "run", query = "") {
    this.help = kind === "help";
    byId("palette-title").textContent = t(this.help ? "palette.helpTitle" : "palette.title");
    this.input.placeholder = t(this.help ? "palette.helpPlaceholder" : "palette.placeholder");
    byId("palette-hint").textContent = t(this.help ? "palette.helpKeys" : "palette.keys");
    this.input.value = query;
    if (!this.dialog.open) this.dialog.showModal();
    this.render();
    this.input.focus();
  }

  render() {
    this.results = searchActions(this.actions, this.input.value);
    this.active = 0;
    const shortcuts = this.shortcuts();
    this.list.replaceChildren(
      ...this.results.map((action, index) => {
        const reason = this.runner.unavailable(action);
        const shortcut = action.shortcut ? shortcuts[action.shortcut] : null;
        const item = el(
          "li",
          {
            id: `palette-item-${index}`,
            class: "palette-item",
            role: "option",
            "data-action": action.id,
            "aria-disabled": reason ? "true" : null,
            onclick: (event) => this.choose(index, event.shiftKey),
          },
          el(
            "span",
            { class: "palette-line" },
            el("span", { class: "palette-name" }, action.title),
            el("span", { class: "palette-group" }, action.group),
            shortcut ? el("kbd", {}, shortcutLabel(shortcut)) : "",
          ),
          el("span", { class: "palette-help" }, reason ?? action.help),
          el(
            "button",
            {
              type: "button",
              class: "btn ghost small palette-show",
              tabindex: "-1",
              onclick: (event) => {
                event.stopPropagation();
                this.choose(index, !this.help);
              },
            },
            t("palette.show"),
          ),
        );
        return item;
      }),
    );
    byId("palette-empty").hidden = this.results.length > 0;
    this.highlight();
  }

  highlight() {
    const items = [...this.list.children];
    items.forEach((item, index) => item.setAttribute("aria-selected", String(index === this.active)));
    const current = items[this.active];
    if (current) {
      this.input.setAttribute("aria-activedescendant", current.id);
      current.scrollIntoView({ block: "nearest" });
    } else {
      this.input.removeAttribute("aria-activedescendant");
    }
  }

  onKey(event) {
    const count = this.results.length;
    if (event.key === "ArrowDown" || event.key === "ArrowUp") {
      if (count === 0) return;
      const step = event.key === "ArrowDown" ? 1 : -1;
      this.active = (this.active + step + count) % count;
      this.highlight();
      event.preventDefault();
    } else if (event.key === "Enter") {
      event.preventDefault();
      if (count > 0) this.choose(this.active, event.shiftKey);
    }
  }

  /**
   * Exécute l'action, ou montre son outil (« Comment faire ? », Maj+Entrée).
   * `other` inverse le choix par défaut de la palette ouverte.
   */
  choose(index, other = false) {
    const action = this.results[index];
    if (!action) return;
    const show = this.help !== other;
    const reason = this.runner.unavailable(action);
    if (!show && reason) {
      toast(reason, "error");
      return;
    }
    this.dialog.close();
    if (!show) {
      this.runner.run(action);
    } else if (this.runner.show(action)) {
      toast(`${action.title} : ${action.help}`);
    } else {
      toast(reason ?? t("palette.unavailable"), "error");
    }
  }
}
