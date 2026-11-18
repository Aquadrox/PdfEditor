// Modification par consigne (IA-09) : lignes modifiées proposées par
// pde-app, montrées en différences mot à mot et encadrées sur les pages ;
// acceptées ou refusées ligne par ligne, posées en une seule opération
// (une annulation les défait toutes). Rien n'est posé sans « Appliquer ».

import { toast } from "../../components/toast.js";
import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { percentBox } from "../viewer/text-geometry.js";
import { wordDiff } from "./ai-diff.js";

export class RewriteView {
  /**
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {{ applied: (pages: object) => Promise<void> }} hooks
   */
  constructor(viewer, hooks) {
    this.viewer = viewer;
    this.hooks = hooks;
    this.pending = null;
  }

  /** Propositions de `result` sous la réponse `entry`. */
  show(entry, result, id) {
    this.clear();
    const box = el("div", { class: "ai-proposals" });
    entry.item.append(box);
    if (result.proposals.length === 0) {
      box.append(el("p", { class: "caption" }, t("rewrite.none")));
      return;
    }
    const rows = result.proposals.map((proposal, index) => {
      const check = el("input", { type: "checkbox", checked: true, "data-rewrite": index, onchange: () => this.preview() });
      const diff = el(
        "span",
        { class: "ai-diff" },
        ...wordDiff(proposal.before, proposal.after).map((part) =>
          part.kind === "same" ? part.text : el(part.kind, {}, part.text),
        ),
      );
      const go = el("button", { class: "btn ghost small", type: "button", onclick: () => this.viewer.goToPage(proposal.page) }, t("assistant.see"));
      const label = el("label", { class: "ai-proposal" }, check, el("span", {}, el("span", { class: "caption" }, t("rewrite.page", { page: proposal.page + 1 })), " ", diff));
      return { proposal, check, row: el("li", {}, label, go) };
    });
    const apply = el("button", { class: "btn ai-apply", type: "button", onclick: () => this.apply() }, t("assistant.apply"));
    const all = el("button", { class: "btn ghost small", type: "button", onclick: () => this.checkAll() }, t("rewrite.all"));
    const refuse = el("button", { class: "btn ghost", type: "button", onclick: () => this.close(t("assistant.refused")) }, t("assistant.refuse"));
    box.append(el("ul", { class: "ai-proposal-list" }, ...rows.map((r) => r.row)), el("div", { class: "ai-proposal-actions" }, all, refuse, apply));
    this.pending = { id, box, rows, apply };
    this.preview();
    this.viewer.goToPage(result.proposals[0].page);
  }

  selected() {
    return this.pending?.rows.filter((r) => r.check.checked).map((r) => r.proposal) ?? [];
  }

  checkAll() {
    const rows = this.pending?.rows ?? [];
    const every = rows.every((r) => r.check.checked);
    for (const row of rows) row.check.checked = !every;
    this.preview();
  }

  /** Cadres en pointillés sur les lignes cochées. */
  preview() {
    this.removePreviews();
    if (!this.pending) return;
    const chosen = this.selected();
    this.pending.apply.disabled = chosen.length === 0;
    this.pending.apply.textContent = t("rewrite.apply", { count: chosen.length });
    for (const proposal of chosen) {
      const frame = this.viewer.frames[proposal.page];
      const size = this.viewer.doc?.pages[proposal.page];
      if (!frame || !size) continue;
      const mark = el("div", { class: "ai-preview rewrite", "aria-hidden": "true" });
      Object.assign(mark.style, percentBox(proposal.bounds, size));
      frame.append(mark);
    }
  }

  removePreviews() {
    for (const mark of document.querySelectorAll(".ai-preview.rewrite")) mark.remove();
  }

  async apply() {
    const pending = this.pending;
    const chosen = this.selected();
    if (!pending || chosen.length === 0) return;
    pending.apply.disabled = true;
    try {
      const accepted = chosen.map((p) => ({ page: p.page, objects: p.objects, before: p.before, text: p.after }));
      const result = await invoke("ai_rewrite_apply", { id: pending.id, accepted });
      const overflow = result.overflowing ? ` ${t("translate.overflowing", { count: result.overflowing })}` : "";
      this.close(`${t("rewrite.applied", { count: result.blocks })}${overflow}`);
      await this.hooks.applied(result.pages);
    } catch (error) {
      pending.apply.disabled = false;
      toast(t("assistant.failed", { message: error?.message ?? String(error) }), "error");
    }
  }

  /** Fin des propositions : la liste reste lisible, sans boutons. */
  close(message) {
    if (!this.pending) return;
    for (const { check } of this.pending.rows) check.disabled = true;
    this.pending.box.querySelector(".ai-proposal-actions")?.replaceChildren(el("p", { class: "caption", role: "status" }, message));
    this.pending = null;
    this.removePreviews();
  }

  clear() {
    this.close(t("assistant.expired"));
    this.removePreviews();
  }
}
