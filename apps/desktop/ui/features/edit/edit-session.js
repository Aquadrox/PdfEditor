// Session d'édition du document ouvert : état renvoyé par pde-app
// (EditView), élément sélectionné, appels aux commandes d'édition. Les
// autres modules s'abonnent aux changements ; quand le plan des pages
// change (mode Pages, ou son annulation), le document est décrit à nouveau
// avant que les abonnés ne reçoivent le nouvel état.

import { invoke } from "../../lib/ipc.js";
import { isExisting, isSynthetic, syntheticItems, withoutImage } from "./existing-image.js";
import { blockItems, isOriginalText, isRemovedText, withNewText } from "./original-text.js";

const DEFAULT_STYLE = {
  font: "sans",
  size: 11,
  bold: false,
  italic: false,
  color: { r: 0x1c, g: 0x24, b: 0x30 },
  align: "left",
  line_height: 1.25,
};

/** Image ou texte d'origine supprimé : l'élément reste (annulable). */
const emptied = (element) => (isOriginalText(element) ? withNewText(element, "") : withoutImage(element));

export class EditSession {
  constructor() {
    this.doc = null;
    this.view = { elements: [], revisions: [], layout: 0, can_undo: false, can_redo: false, dirty: false };
    /** Élément principal de la sélection (panneau, poignées). */
    this.selected = null;
    /** Autres éléments sélectionnés (Maj+clic, Ctrl+A). */
    this.extra = new Set();
    /** Style des nouvelles zones : celui de la dernière zone modifiée. */
    this.style = { ...DEFAULT_STYLE };
    this.listeners = [];
    this.layoutListeners = [];
    /** Zones caviardées dont l'écriture a été confirmée (saving.js). */
    this.confirmedRedactions = new Set();
    /** Images d'origine de chaque page déjà lue : [{ object, bounds }]. */
    this.pageImages = new Map();
    /** Blocs de texte d'origine de chaque page déjà lue : [{ objects, bounds, text, font_name, style }]. */
    this.pageTextBlocks = new Map();
    this.loading = new Map();
  }

  /**
   * Objets d'origine d'une page (`page_images` ou `page_text_blocks`),
   * demandés une seule fois ; `true` quand ils viennent d'arriver (la
   * couche de la page est à redessiner).
   */
  async loadPageObjects(command, page) {
    const cache = command === "page_images" ? this.pageImages : this.pageTextBlocks;
    if (!this.doc || cache.has(page)) return false;
    const key = `${command}:${page}`;
    if (!this.loading.has(key)) {
      const doc = this.doc;
      const loading = invoke(command, { id: doc.id, page })
        .then((objects) => {
          if (this.doc === doc) cache.set(page, objects ?? []);
        })
        .finally(() => this.loading.delete(key));
      this.loading.set(key, loading);
      await loading;
      return cache.has(page);
    }
    await this.loading.get(key);
    return false;
  }

  loadPageImages(page) {
    return this.loadPageObjects("page_images", page);
  }

  loadPageTextBlocks(page) {
    return this.loadPageObjects("page_text_blocks", page);
  }

  /**
   * Éléments d'une page à montrer : les ajouts (sauf les images et les
   * textes d'origine supprimés) et les images d'origine encore intactes.
   */
  itemsOn(page) {
    const items = this.view.elements.filter((e) => e.element.page === page);
    const shown = items.filter((e) => !(isExisting(e.element) && e.element.frame === null) && !isRemovedText(e.element));
    return [...shown, ...syntheticItems(page, this.pageImages.get(page) ?? [], items)];
  }

  /** Lignes de texte d'origine de la page encore à proposer (outil Texte du document). */
  blocksOn(page) {
    const items = this.view.elements.filter((e) => e.element.page === page);
    return blockItems(page, this.pageTextBlocks.get(page) ?? [], items);
  }

  /** `listener(document)` quand le nombre, l'ordre ou la taille des pages change. */
  onLayout(listener) {
    this.layoutListeners.push(listener);
  }

  /** `listener(view, changedPages)` à chaque changement. */
  subscribe(listener) {
    this.listeners.push(listener);
  }

  get dirty() {
    return Boolean(this.doc && this.view.dirty);
  }

  async load(doc) {
    this.doc = doc;
    this.confirmedRedactions.clear();
    this.pageImages.clear();
    this.pageTextBlocks.clear();
    this.selected = null;
    this.extra.clear();
    this.update(doc ? await invoke("edit_view", { id: doc.id }) : { elements: [], revisions: [] });
  }

  /** État relu après un enregistrement (« modifié » remis à zéro). */
  async reload() {
    if (this.doc) this.update(await invoke("edit_view", { id: this.doc.id }));
  }

  element(id) {
    if (isSynthetic(id)) {
      const page = Number(id.split(":")[1]);
      return this.itemsOn(page).find((e) => e.id === id) ?? null;
    }
    return this.view.elements.find((e) => e.id === id) ?? null;
  }

  /** Sélection : élément principal d'abord. */
  get selection() {
    return this.selected === null ? [] : [this.selected, ...this.extra];
  }

  isSelected(id) {
    return id === this.selected || this.extra.has(id);
  }

  /** Sélection d'un seul élément (ou aucune avec null). */
  select(id) {
    this.selected = id;
    this.extra.clear();
    this.notify([]);
  }

  /** Plusieurs éléments ; le premier devient le principal. */
  selectMany(ids) {
    this.selected = ids[0] ?? null;
    this.extra = new Set(ids.slice(1));
    this.notify([]);
  }

  /** Maj+clic : ajoute l'élément à la sélection, ou l'en retire. */
  toggle(id) {
    if (id === this.selected) {
      const [next, ...rest] = this.extra;
      this.selected = next ?? null;
      this.extra = new Set(rest);
    } else if (this.extra.has(id)) {
      this.extra.delete(id);
    } else if (this.selected === null) {
      this.selected = id;
    } else {
      this.extra.add(id);
    }
    this.notify([]);
  }

  /** Plusieurs éléments ajoutés en une opération ; ils deviennent la sélection. */
  async addMany(elements) {
    const added = await invoke("add_elements", { id: this.doc.id, elements });
    this.selected = added.ids[0] ?? null;
    this.extra = new Set(added.ids.slice(1));
    this.update(added.view);
    return added.ids;
  }

  /**
   * Ajouts, changements et retraits en une seule opération ; renvoie les
   * identifiants des éléments ajoutés.
   */
  async editBatch({ added = [], changes = [], removed = [] }) {
    const payload = changes.map(({ id, element }) => ({ elementId: id, element }));
    const result = await invoke("edit_elements", { id: this.doc.id, added, changes: payload, removed });
    this.update(result.view);
    return result.ids;
  }

  /** Plusieurs éléments changés en une opération : [{ id, element }]. */
  async changeMany(changes) {
    if (changes.length === 0) return;
    if (changes.length === 1) return this.change(changes[0].id, changes[0].element);
    // Images d'origine pas encore modifiées : leur élément est créé.
    if (changes.some((c) => isSynthetic(c.id))) {
      const fresh = changes.filter((c) => isSynthetic(c.id));
      const ids = await this.editBatch({
        added: fresh.map((c) => c.element),
        changes: changes.filter((c) => !isSynthetic(c.id)),
      });
      const renamed = new Map(fresh.map((c, i) => [c.id, ids[i]]));
      this.selectMany(this.selection.map((id) => renamed.get(id) ?? id));
      return;
    }
    const payload = changes.map(({ id, element }) => ({ elementId: id, element }));
    this.update(await invoke("update_elements", { id: this.doc.id, changes: payload }));
  }

  async removeMany(ids) {
    if (ids.length === 1) return this.remove(ids[0]);
    // Images d'origine : supprimées par leur élément (annulable), pas retirées.
    // Texte d'origine : vidé (annulable), pas retiré.
    const items = ids.map((id) => this.element(id)).filter(Boolean);
    const kept = (i) => isExisting(i.element) || isOriginalText(i.element);
    if (items.some(kept)) {
      this.selected = null;
      this.extra.clear();
      await this.editBatch({
        added: items.filter((i) => i.synthetic).map((i) => withoutImage(i.element)),
        changes: items.filter((i) => !i.synthetic && kept(i)).map((i) => ({ id: i.id, element: emptied(i.element) })),
        removed: items.filter((i) => !kept(i)).map((i) => i.id),
      });
      return;
    }
    this.selected = null;
    this.extra.clear();
    this.update(await invoke("remove_elements", { id: this.doc.id, elementIds: ids }));
  }

  async add(element) {
    const added = await invoke("add_element", { id: this.doc.id, element });
    if (element.kind === "text") this.style = { ...element.style };
    this.selected = added.id;
    this.extra.clear();
    this.update(added.view);
    return added.id;
  }

  async change(id, element) {
    // Première modification d'une image d'origine : son élément est créé.
    if (isSynthetic(id)) return this.add(element);
    if (element.kind === "text") this.style = { ...element.style };
    this.update(await invoke("update_element", { id: this.doc.id, elementId: id, element }));
  }

  async remove(id) {
    const item = this.element(id);
    if (this.selected === id) this.selected = null;
    if (item && (isExisting(item.element) || isOriginalText(item.element))) {
      const element = emptied(item.element);
      const view = item.synthetic
        ? (await invoke("add_element", { id: this.doc.id, element })).view
        : await invoke("update_element", { id: this.doc.id, elementId: id, element });
      return this.update(view);
    }
    this.update(await invoke("remove_element", { id: this.doc.id, elementId: id }));
  }

  async undo() {
    await this.apply(await invoke("undo_edit", { id: this.doc.id }));
  }

  async redo() {
    await this.apply(await invoke("redo_edit", { id: this.doc.id }));
  }

  /**
   * Nouvel état après une opération qui peut changer le plan des pages :
   * le document (`document`, sinon relu) est d'abord transmis aux abonnés
   * de `onLayout`.
   */
  async apply(view, document = null) {
    if (this.doc && view.layout !== this.view.layout) {
      this.pageImages.clear();
      this.pageTextBlocks.clear();
      const described = document ?? (await invoke("document_view", { id: this.doc.id }));
      for (const listener of this.layoutListeners) listener(described);
    }
    this.update(view);
  }

  /** Nouvel état ; les pages dont la version change sont signalées. */
  update(view) {
    const before = this.view.revisions;
    const changed = view.revisions.flatMap((revision, page) => (revision !== before[page] ? [page] : []));
    this.view = view;
    for (const id of this.extra) if (!this.element(id)) this.extra.delete(id);
    if (this.selected !== null && !this.element(this.selected)) {
      const [next, ...rest] = this.extra;
      this.selected = next ?? null;
      this.extra = new Set(rest);
    }
    this.notify(changed);
  }

  notify(changedPages) {
    for (const listener of this.listeners) listener(this.view, changedPages);
  }
}
