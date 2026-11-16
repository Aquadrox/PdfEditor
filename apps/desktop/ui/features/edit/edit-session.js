// Session d'édition du document ouvert : état renvoyé par pde-app
// (EditView), élément sélectionné, appels aux commandes d'édition. Les
// autres modules s'abonnent aux changements.

import { invoke } from "../../lib/ipc.js";

const DEFAULT_STYLE = {
  font: "sans",
  size: 11,
  bold: false,
  italic: false,
  color: { r: 0x1c, g: 0x24, b: 0x30 },
  align: "left",
  line_height: 1.25,
};

export class EditSession {
  constructor() {
    this.doc = null;
    this.view = { elements: [], revisions: [], can_undo: false, can_redo: false, dirty: false };
    this.selected = null;
    /** Style des nouvelles zones : celui de la dernière zone modifiée. */
    this.style = { ...DEFAULT_STYLE };
    this.listeners = [];
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
    this.selected = null;
    this.update(doc ? await invoke("edit_view", { id: doc.id }) : { elements: [], revisions: [] });
  }

  /** État relu après un enregistrement (« modifié » remis à zéro). */
  async reload() {
    if (this.doc) this.update(await invoke("edit_view", { id: this.doc.id }));
  }

  element(id) {
    return this.view.elements.find((e) => e.id === id) ?? null;
  }

  select(id) {
    this.selected = id;
    this.notify([]);
  }

  async add(element) {
    const added = await invoke("add_element", { id: this.doc.id, element });
    if (element.style) this.style = { ...element.style };
    this.selected = added.id;
    this.update(added.view);
    return added.id;
  }

  async change(id, element) {
    if (element.style) this.style = { ...element.style };
    this.update(await invoke("update_element", { id: this.doc.id, elementId: id, element }));
  }

  async remove(id) {
    if (this.selected === id) this.selected = null;
    this.update(await invoke("remove_element", { id: this.doc.id, elementId: id }));
  }

  async undo() {
    this.update(await invoke("undo_edit", { id: this.doc.id }));
  }

  async redo() {
    this.update(await invoke("redo_edit", { id: this.doc.id }));
  }

  /** Nouvel état ; les pages dont la version change sont signalées. */
  update(view) {
    const before = this.view.revisions;
    const changed = view.revisions.flatMap((revision, page) => (revision !== before[page] ? [page] : []));
    this.view = view;
    if (this.selected !== null && !this.element(this.selected)) this.selected = null;
    this.notify(changed);
  }

  notify(changedPages) {
    for (const listener of this.listeners) listener(this.view, changedPages);
  }
}
