// Barre de recherche (V-05) : Ctrl+F, Entrée / Maj+Entrée, Échap.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { countLabel, firstFrom, step } from "./search-state.js";

/** Délai entre la dernière frappe et la recherche (ms). */
const TYPING_DELAY = 250;

export function attachSearch(viewer) {
  const bar = byId("search-bar");
  const input = byId("search-input");
  const count = byId("search-count");
  let results = { hits: [], truncated: false };
  let index = -1;
  let timer = 0;
  let generation = 0;

  function show() {
    count.textContent = input.value.trim() ? countLabel(index, results.hits.length, results.truncated) : "";
    viewer.setHighlights(results.hits, index);
  }

  async function run() {
    timer = 0;
    const query = input.value;
    const asked = ++generation;
    if (!viewer.doc || !query.trim()) {
      results = { hits: [], truncated: false };
      index = -1;
      show();
      return;
    }
    try {
      const found = await invoke("search_document", { id: viewer.doc.id, query });
      // Une frappe plus récente a relancé la recherche : résultat périmé.
      if (asked !== generation) return;
      results = found;
      index = firstFrom(found.hits, viewer.page);
      show();
    } catch (error) {
      toast(t("search.failed", { message: error?.message ?? String(error) }), "error");
    }
  }

  function move(direction) {
    if (results.hits.length === 0) return;
    index = step(index, results.hits.length, direction);
    show();
  }

  function open() {
    if (!viewer.doc) return;
    bar.hidden = false;
    input.focus();
    input.select();
  }

  function close() {
    bar.hidden = true;
    clearTimeout(timer);
    generation += 1;
    results = { hits: [], truncated: false };
    index = -1;
    viewer.setHighlights([], -1);
    viewer.desk.focus({ preventScroll: true });
  }

  input.addEventListener("input", () => {
    clearTimeout(timer);
    timer = setTimeout(run, TYPING_DELAY);
  });
  input.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      event.preventDefault();
      // Recherche en attente : la lancer tout de suite plutôt que d'avancer.
      if (timer) {
        clearTimeout(timer);
        timer = 0;
        run();
      } else move(event.shiftKey ? -1 : 1);
    } else if (event.key === "Escape") {
      event.preventDefault();
      close();
    }
  });
  byId("search-next").addEventListener("click", () => move(1));
  byId("search-previous").addEventListener("click", () => move(-1));
  byId("search-close").addEventListener("click", close);
  byId("search-button").addEventListener("click", () => (bar.hidden ? open() : close()));

  return {
    open,
    /** Nouveau document ou document fermé : la recherche repart de zéro. */
    reset() {
      input.value = "";
      bar.hidden = true;
      clearTimeout(timer);
      generation += 1;
      results = { hits: [], truncated: false };
      index = -1;
      count.textContent = "";
      byId("search-button").disabled = !viewer.doc;
    },
  };
}
