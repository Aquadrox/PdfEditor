// Rubrique « Recettes » des Réglages (IA-16) : recettes intégrées (lecture
// seule, à dupliquer) et personnelles (créer, modifier, supprimer),
// import et export de fichiers. Enregistrées tout de suite, à part du
// bouton « Enregistrer » des réglages.

import { toast } from "../../components/toast.js";
import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";

/** @param {(recipes: object[]) => void} onChange liste à jour (mode IA) */
export function attachRecipesSettings(onChange) {
  let recipes = [];
  /** Recette affichée : { id, builtin } ; id null pour une nouvelle. */
  let current = null;
  const form = byId("recipe-form");
  const actions = () => [...form.querySelectorAll(".recipe-actions input")];

  function fail(error) {
    byId("recipe-error").textContent = error?.message ?? String(error);
  }

  async function refresh(selectId = current?.id) {
    recipes = (await invoke("recipes_list").catch(() => null)) ?? [];
    onChange(recipes);
    const pick = byId("recipe-pick");
    pick.replaceChildren(...recipes.map((r) => el("option", { value: r.id }, r.recipe.name)));
    const shown = recipes.find((r) => r.id === selectId) ?? recipes[0];
    if (shown) show(shown);
  }

  function show(view) {
    current = { id: view.id, builtin: view.builtin };
    byId("recipe-pick").value = view.id ?? "";
    fill(view.recipe, view.builtin);
  }

  function fill(recipe, builtin) {
    byId("recipe-name").value = recipe.name;
    byId("recipe-description").value = recipe.description ?? "";
    byId("recipe-instruction").value = recipe.instruction;
    byId("recipe-scope").value = recipe.scope ?? "document";
    for (const box of actions()) box.checked = recipe.actions.includes(box.value);
    for (const field of form.querySelectorAll("input, textarea, select")) field.disabled = builtin;
    byId("recipe-builtin").hidden = !builtin;
    byId("recipe-save").hidden = builtin;
    byId("recipe-delete").hidden = builtin || current?.id === null;
    byId("recipe-export").hidden = current?.id === null;
    byId("recipe-error").textContent = "";
  }

  function read() {
    return {
      name: byId("recipe-name").value.trim(),
      description: byId("recipe-description").value.trim(),
      instruction: byId("recipe-instruction").value.trim(),
      actions: actions().filter((box) => box.checked).map((box) => box.value),
      scope: byId("recipe-scope").value,
    };
  }

  /** Nouvelle recette (vide, ou copie d'une autre), pas encore enregistrée. */
  function draft(recipe) {
    current = { id: null, builtin: false };
    fill(recipe, false);
    byId("recipe-name").focus();
    byId("recipe-name").select();
  }

  byId("recipe-pick").addEventListener("change", (event) => {
    const view = recipes.find((r) => r.id === event.target.value);
    if (view) show(view);
  });
  byId("recipe-new").addEventListener("click", () =>
    draft({ name: t("recipes.untitled"), description: "", instruction: "", actions: ["highlight"], scope: "document" }),
  );
  byId("recipe-duplicate").addEventListener("click", () => {
    const recipe = { ...read(), name: t("recipes.copy", { name: byId("recipe-name").value.trim() }) };
    draft(recipe);
  });
  form.addEventListener("submit", async (event) => {
    event.preventDefault();
    if (current?.builtin) return;
    const recipe = read();
    if (recipe.actions.length === 0) return fail({ message: t("recipes.noAction") });
    try {
      const saved = await invoke("recipe_save", { id: current?.id ?? null, recipe });
      toast(t("recipes.saved", { name: saved.recipe.name }));
      await refresh(saved.id);
    } catch (error) {
      fail(error);
    }
  });
  byId("recipe-delete").addEventListener("click", async () => {
    if (!current?.id || current.builtin) return;
    try {
      await invoke("recipe_delete", { id: current.id });
      toast(t("recipes.deleted"));
      await refresh(null);
    } catch (error) {
      fail(error);
    }
  });
  byId("recipe-import").addEventListener("click", async () => {
    try {
      const imported = await invoke("recipe_import");
      if (!imported) return;
      toast(t("recipes.imported", { name: imported.recipe.name }));
      await refresh(imported.id);
    } catch (error) {
      fail(error);
    }
  });
  byId("recipe-export").addEventListener("click", async () => {
    if (!current?.id) return;
    try {
      if (await invoke("recipe_export", { id: current.id, name: byId("recipe-name").value })) toast(t("recipes.exported"));
    } catch (error) {
      fail(error);
    }
  });

  return { refresh };
}
