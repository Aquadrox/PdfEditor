// Page Réglages : rubriques, champs liés au brouillon, enregistrement.
// Chaque champ porte data-setting="rubrique.clé" (et data-type si besoin).

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { countChanges, getPath, setPath } from "./settings-state.js";
import { renderShortcuts } from "./shortcuts-editor.js";

/**
 * @param {{ onSaved: (settings: object) => void, onClose: () => void }} hooks
 */
export function attachSettings(hooks) {
  const view = byId("settings-view");
  const fields = [...view.querySelectorAll("[data-setting]")];
  let saved = null;
  let draft = null;

  function read(field) {
    if (field.type === "checkbox") return field.checked;
    if (field.dataset.type === "number") return Number(field.value);
    return field.value;
  }

  function fill() {
    for (const field of fields) {
      const value = getPath(draft, field.dataset.setting);
      if (field.type === "checkbox") field.checked = Boolean(value);
      else {
        // Valeur écrite à la main dans config.toml, absente de la liste.
        if (field.tagName === "SELECT" && ![...field.options].some((o) => o.value === String(value))) {
          field.append(new Option(t("settings.autosave.custom", { seconds: value }), String(value)));
        }
        field.value = String(value);
      }
    }
    drawShortcuts();
    refresh();
  }

  function drawShortcuts() {
    renderShortcuts(
      byId("shortcuts-body"),
      () => draft.shortcuts,
      (action, shortcut) => {
        draft = setPath(draft, `shortcuts.${action}`, shortcut);
        drawShortcuts();
        refresh();
      },
    );
  }

  function refresh() {
    const count = countChanges(saved, draft);
    byId("settings-dirty").textContent = count > 0 ? t("settings.changes", { count }) : "";
    byId("settings-save").disabled = count === 0;
  }

  function showSection(name) {
    for (const button of view.querySelectorAll(".settings-nav [data-section]")) {
      if (button.dataset.section === name) button.setAttribute("aria-current", "page");
      else button.removeAttribute("aria-current");
    }
    for (const section of view.querySelectorAll(".settings-section")) {
      section.hidden = section.dataset.section !== name;
    }
  }

  for (const field of fields) {
    field.addEventListener("change", () => {
      draft = setPath(draft, field.dataset.setting, read(field));
      refresh();
    });
  }
  for (const button of view.querySelectorAll(".settings-nav [data-section]")) {
    button.addEventListener("click", () => showSection(button.dataset.section));
  }
  byId("shortcuts-reset").addEventListener("click", async () => {
    const defaults = await invoke("default_settings");
    draft = setPath(draft, "shortcuts", defaults.shortcuts);
    drawShortcuts();
    refresh();
  });
  byId("settings-cancel").addEventListener("click", () => {
    draft = structuredClone(saved);
    fill();
  });
  byId("settings-save").addEventListener("click", async () => {
    try {
      saved = await invoke("save_settings", { settings: draft });
      draft = structuredClone(saved);
      fill();
      hooks.onSaved(saved);
      toast(t("settings.saved"));
    } catch (error) {
      toast(t("settings.failed", { message: error?.message ?? String(error) }), "error");
    }
  });
  byId("settings-back").addEventListener("click", () => hooks.onClose());
  byId("open-logs").addEventListener("click", () => invoke("open_logs_folder").catch(() => {}));
  // Dossier par défaut : choisi dans le dialogue du système.
  byId("pick-default-dir").addEventListener("click", async () => {
    const folder = await invoke("pick_folder").catch(() => null);
    if (!folder) return;
    const input = byId("set-default-dir");
    input.value = folder;
    input.dispatchEvent(new Event("change"));
  });

  return {
    /** Réglages lus au démarrage (SettingsView de pde-app). */
    load(settingsView) {
      saved = settingsView.settings;
      draft = structuredClone(saved);
      const warnings = byId("settings-warnings");
      warnings.textContent = settingsView.warnings.join(" ");
      warnings.hidden = settingsView.warnings.length === 0;
      fill();
    },
    open() {
      showSection("general");
      view.querySelector(".settings-nav [data-section]")?.focus();
    },
  };
}
