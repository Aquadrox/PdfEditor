// Rubrique IA des Réglages (R-07, IA-01, IA-02) : préréglages, clé API au
// trousseau (jamais affichée), test des réglages et liste des modèles,
// profils et profil de secours, consommation.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { PRESETS, applyPreset, presetFor } from "./ai-presets.js";

const message = (error) => error?.message ?? String(error);

function money(value, currency) {
  return `${Number(value ?? 0).toFixed(value >= 1 ? 2 : 4)} ${currency}`;
}

/**
 * @param {{ draft: () => object, set: (path: string, value: unknown) => void }} settings
 * page Réglages (brouillon)
 */
export function attachAiSettings(settings) {
  /**
   * Modèles du serveur, en boutons (comme VoiceToText) : un clic les met
   * dans le champ Modèle, le modèle choisi est marqué.
   */
  function modelChoice(models) {
    const current = settings.draft().ai.provider.model;
    const buttons = models.map((name) =>
      el(
        "button",
        {
          type: "button",
          class: `btn small ${name === current ? "on" : "ghost"}`,
          "aria-pressed": String(name === current),
          title: t("settings.ai.useModel"),
          onclick: (event) => {
            settings.set("ai.provider.model", name);
            byId("set-ai-model").value = name;
            for (const other of event.currentTarget.parentElement.children) {
              const on = other === event.currentTarget;
              other.classList.toggle("on", on);
              other.classList.toggle("ghost", !on);
              other.setAttribute("aria-pressed", String(on));
            }
          },
        },
        name,
      ),
    );
    return el(
      "div",
      { class: "ai-models" },
      el("p", { class: "caption" }, t("settings.ai.models", { count: models.length })),
      el("div", { class: "ai-model-list" }, ...buttons),
    );
  }

  const preset = byId("ai-preset");
  preset.replaceChildren(...PRESETS.map((p) => el("option", { value: p.id }, p.label)));
  preset.addEventListener("change", () => {
    const chosen = PRESETS.find((p) => p.id === preset.value);
    byId("ai-preset-note").textContent = chosen?.note ?? "";
    if (chosen?.base_url) settings.set("ai.provider", applyPreset(settings.draft().ai.provider, chosen));
  });

  byId("ai-key-save").addEventListener("click", async () => {
    const input = byId("ai-key");
    const key = input.value;
    try {
      await invoke("ai_set_key", { provider: settings.draft().ai.provider, key });
      input.value = "";
      // Clé saisie : enregistrée ; champ vide : clé retirée.
      let status = "settings.ai.keyRemoved";
      if (key.trim()) status = "settings.ai.keyStored";
      byId("ai-key-status").textContent = t(status);
    } catch (error) {
      byId("ai-key-status").textContent = message(error);
    }
  });

  byId("ai-test").addEventListener("click", async () => {
    const box = byId("ai-test-result");
    box.hidden = false;
    box.classList.remove("ok", "failed");
    box.textContent = t("settings.ai.testing");
    try {
      const result = await invoke("ai_test", { provider: settings.draft().ai.provider });
      box.classList.add(result.ok ? "ok" : "failed");
      box.replaceChildren(
        el("strong", {}, result.ok ? t("settings.ai.testOk") : t("settings.ai.testFailed")),
        el("span", {}, ` ${result.message}`),
        ...result.hints.map((hint) => el("p", { class: "caption" }, hint)),
      );
      byId("ai-models").replaceChildren(...result.models.map((m) => el("option", { value: m })));
      if (result.models.length) box.append(modelChoice(result.models));
    } catch (error) {
      box.classList.add("failed");
      box.textContent = message(error);
    }
  });

  byId("ai-profile-save").addEventListener("click", () => {
    const name = byId("ai-profile-name").value.trim();
    if (!name) return;
    const draft = settings.draft();
    const profiles = draft.ai.profiles.filter((p) => p.name.toLowerCase() !== name.toLowerCase());
    profiles.push({ name, ...draft.ai.provider });
    byId("ai-profile-name").value = "";
    settings.set("ai.profiles", profiles);
  });
  byId("ai-profile-use").addEventListener("click", () => {
    const profile = settings.draft().ai.profiles.find((p) => p.name === byId("ai-profile").value);
    if (!profile) return;
    const { name: _name, ...provider } = profile;
    settings.set("ai.provider", provider);
  });
  byId("ai-profile-delete").addEventListener("click", () => {
    const name = byId("ai-profile").value;
    const draft = settings.draft();
    settings.set("ai.profiles", draft.ai.profiles.filter((p) => p.name !== name));
    if (draft.ai.fallback === name) settings.set("ai.fallback", "");
  });

  return {
    /** Avant le remplissage des champs : listes tirées du brouillon. */
    fill(draft) {
      const ai = draft.ai;
      const chosen = presetFor(ai.provider.base_url);
      preset.value = chosen.id;
      byId("ai-preset-note").textContent = chosen.note;
      const names = ai.profiles.map((p) => p.name);
      byId("ai-profile").replaceChildren(...names.map((n) => el("option", { value: n }, n)));
      byId("ai-profile-use").disabled = names.length === 0;
      byId("ai-profile-delete").disabled = names.length === 0;
      byId("set-ai-fallback").replaceChildren(
        el("option", { value: "" }, t("settings.ai.noFallback")),
        ...names.map((n) => el("option", { value: n }, n)),
      );
      this.refresh(ai);
    },
    /** Clé enregistrée et consommation (jamais la clé elle-même). */
    async refresh(ai) {
      const status = await invoke("ai_status").catch(() => null);
      if (status) {
        byId("ai-key-status").textContent = status.local
          ? t("settings.ai.keyLocal")
          : status.has_key
            ? t("settings.ai.keyStored")
            : t("settings.ai.keyMissing");
      }
      const usage = await invoke("ai_usage").catch(() => null);
      if (usage) {
        const currency = ai.provider.pricing.currency || "USD";
        byId("ai-usage").textContent = t("settings.ai.usageText", {
          today: money(usage.today.cost, currency),
          month: money(usage.month.cost, currency),
          total: money(usage.total.cost, currency),
          requests: usage.total.requests,
        });
      }
    },
  };
}
