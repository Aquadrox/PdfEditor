// Assistant d'actions (IA-15) : propositions vérifiées par pde-app,
// montrées en aperçu sur les pages (cadres en pointillés), puis appliquées
// sur accord en une seule opération (une annulation les défait toutes).
// Rien n'est appliqué sans clic sur « Appliquer ».

import { toast } from "../../components/toast.js";
import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { percentBox } from "../viewer/text-geometry.js";

/**
 * Portée d'une modification du texte : les pages nommées par chaque
 * proposition (rangs à partir de 0), sinon la portée de la demande.
 */
export function rewriteScope(edits, scope) {
  if (edits.some((e) => e.proposal.pages.length === 0)) return scope;
  const pages = [...new Set(edits.flatMap((e) => e.pages))].sort((a, b) => a - b);
  return { kind: "pages", pages };
}

export class AssistantView {
  /**
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {object} hooks
   * @param {(result: object) => Promise<void>} hooks.applied état de l'édition après application
   * @param {(query: string) => void} hooks.howTo « Comment faire ? » sur la demande
   * @param {(instruction: string, scope: object) => void} hooks.rewrite modification par consigne (IA-09)
   */
  constructor(viewer, hooks) {
    this.viewer = viewer;
    this.hooks = hooks;
    /** Propositions en attente : une seule réponse à la fois. */
    this.pending = null;
  }

  /** Ajoute les propositions de `answer` sous la réponse `entry`. */
  show(entry, answer, context) {
    this.clear();
    // Changer le texte (edit_text) : confié à la modification par consigne,
    // qui propose chaque ligne modifiée ; le reste s'applique ici.
    const edits = (answer.proposals ?? []).filter((p) => p.proposal.action === "edit_text");
    const proposals = (answer.proposals ?? []).filter((p) => p.proposal.action !== "edit_text");
    const rejected = answer.rejected ?? [];
    const box = el("div", { class: "ai-proposals" });
    entry.item.append(box);
    if (edits.length) {
      box.append(el("p", { class: "caption ai-handoff", role: "status" }, t("assistant.rewriting")));
      this.hooks.rewrite(edits.map((e) => e.proposal.instruction.trim()).join("\n"), rewriteScope(edits, context.scope));
      if (proposals.length === 0) {
        box.append(this.rejectedList(rejected));
        return;
      }
    }
    if (proposals.length === 0) {
      box.append(el("p", { class: "caption" }, t(rejected.length ? "assistant.noneValid" : "assistant.none")));
      box.append(this.rejectedList(rejected));
      box.append(el("button", { class: "btn small", type: "button", onclick: () => this.hooks.howTo(context.request) }, t("assistant.howTo")));
      return;
    }
    const checks = proposals.map((proposal, index) => {
      const check = el("input", { type: "checkbox", checked: true, "data-proposal": index, onchange: () => this.preview() });
      const go = el("button", { class: "btn ghost small", type: "button", onclick: () => this.goTo(proposal) }, t("assistant.see"));
      const pages = proposal.pages.length ? ` · ${t("assistant.pages", { pages: proposal.pages.map((p) => p + 1).join(", ") })}` : "";
      const label = el("label", { class: "ai-proposal" }, check, el("span", {}, proposal.summary, el("span", { class: "caption" }, pages)));
      return { proposal, check, row: el("li", {}, label, proposal.previews.length ? go : "") };
    });
    const apply = el("button", { class: "btn ai-apply", type: "button", onclick: () => this.apply() }, t("assistant.apply"));
    const refuse = el("button", { class: "btn ghost", type: "button", onclick: () => this.refuse() }, t("assistant.refuse"));
    box.append(
      el("ul", { class: "ai-proposal-list" }, ...checks.map((c) => c.row)),
      this.rejectedList(rejected),
      proposals.some((p) => p.redaction) ? el("p", { class: "caption ai-redact-note" }, t("assistant.redactNote")) : "",
      el("div", { class: "ai-proposal-actions" }, refuse, apply),
    );
    this.pending = { entry, box, checks, apply, refuse, context };
    this.preview();
    this.goTo(proposals.find((p) => p.previews.length) ?? proposals[0]);
  }

  rejectedList(rejected) {
    if (rejected.length === 0) return "";
    return el(
      "details",
      { class: "ai-rejected" },
      el("summary", {}, t("assistant.rejected", { count: rejected.length })),
      el("ul", {}, ...rejected.map((reason) => el("li", {}, reason))),
    );
  }

  selected() {
    return this.pending?.checks.filter((c) => c.check.checked).map((c) => c.proposal) ?? [];
  }

  /** Cadres en pointillés sur les zones des propositions cochées. */
  preview() {
    this.removePreviews();
    if (!this.pending) return;
    const chosen = this.selected();
    this.pending.apply.disabled = chosen.length === 0;
    for (const proposal of chosen) {
      for (const { page, area } of proposal.previews) {
        const frame = this.viewer.frames[page];
        const size = this.viewer.doc?.pages[page];
        if (!frame || !size) continue;
        const mark = el("div", { class: `ai-preview ${proposal.redaction ? "redact" : ""}`, "aria-hidden": "true" });
        Object.assign(mark.style, percentBox(area, size));
        frame.append(mark);
      }
    }
  }

  removePreviews() {
    for (const mark of document.querySelectorAll(".ai-preview")) mark.remove();
  }

  goTo(proposal) {
    const page = proposal?.previews[0]?.page ?? proposal?.pages[0];
    if (page !== undefined) this.viewer.goToPage(page);
  }

  async apply() {
    const pending = this.pending;
    const chosen = this.selected();
    if (!pending || chosen.length === 0) return;
    pending.apply.disabled = true;
    try {
      const { id, scope } = pending.context;
      const result = await invoke("assistant_apply", { id, scope, proposals: chosen.map((p) => p.proposal) });
      this.close(t("assistant.applied", { count: chosen.length }));
      await this.hooks.applied(result);
    } catch (error) {
      pending.apply.disabled = false;
      toast(t("assistant.failed", { message: error?.message ?? String(error) }), "error");
    }
  }

  refuse() {
    this.close(t("assistant.refused"));
  }

  /** Fin des propositions en attente : la liste reste lisible, sans boutons. */
  close(message) {
    if (!this.pending) return;
    const { box, checks } = this.pending;
    for (const { check } of checks) check.disabled = true;
    box.querySelector(".ai-proposal-actions")?.replaceChildren(el("p", { class: "caption", role: "status" }, message));
    this.pending = null;
    this.removePreviews();
  }

  /** Document fermé ou changé : propositions oubliées. */
  clear() {
    this.close(t("assistant.expired"));
    this.removePreviews();
  }
}
