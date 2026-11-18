// Mode IA (IA-04, IA-05, IA-12) : portée (sélection, page, pages,
// document), analyses et questions, estimation et confirmation avant
// l'envoi, réponse en flux avec citations de pages cliquables, annulation.
// Tout texte venu du document ou du modèle est affiché en textContent.

import { toast } from "../../components/toast.js";
import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke, listen } from "../../lib/ipc.js";
import { blocks, parsePages } from "./ai-text.js";

function money(value, currency) {
  return `${Number(value ?? 0).toFixed(value >= 1 ? 2 : 4)} ${currency}`;
}

export class AiMode {
  /**
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {{ openSettings: () => void }} hooks
   */
  constructor(viewer, hooks) {
    this.viewer = viewer;
    this.hooks = hooks;
    this.doc = null;
    this.active = false;
    this.scope = "document";
    this.selection = null;
    /** Réponses en cours, par numéro d'action. */
    this.runs = new Map();
    for (const button of document.querySelectorAll("[data-scope]")) {
      button.addEventListener("click", () => this.setScope(button.dataset.scope));
    }
    for (const button of document.querySelectorAll("[data-task]")) {
      button.addEventListener("click", () => this.ask({ kind: button.dataset.task }));
    }
    byId("ai-pages").addEventListener("input", () => this.showScope());
    byId("ai-ask").addEventListener("submit", (event) => {
      event.preventDefault();
      const question = byId("ai-question").value.trim();
      if (!question) return;
      this.ask({ kind: "question", question });
    });
    byId("ai-question").addEventListener("keydown", (event) => {
      if (event.key === "Enter" && !event.shiftKey) {
        event.preventDefault();
        byId("ai-ask").requestSubmit();
      }
    });
    byId("ai-open-settings").addEventListener("click", () => hooks.openSettings());
    // Texte sélectionné dans le document : gardé pour la portée « Sélection ».
    document.addEventListener("selectionchange", () => this.captureSelection());
    listen("ai-event", (event) => this.onEvent(event.payload)).catch(() => {});
    listen("ai-done", (event) => this.onDone(event.payload)).catch(() => {});
  }

  load(doc) {
    this.doc = doc;
    this.selection = null;
    byId("ai-answers").replaceChildren();
    this.showScope();
  }

  async setActive(active) {
    this.active = active;
    byId("ai-panel").hidden = !active;
    if (!active) return;
    if (this.selection) this.scope = "selection";
    this.showScope();
    const status = await invoke("ai_status").catch(() => null);
    const configured = Boolean(status?.configured);
    byId("ai-unconfigured").hidden = configured;
    byId("ai-provider").textContent = configured ? status.label : "";
    for (const button of document.querySelectorAll("[data-task], #ai-send")) button.disabled = !configured || !this.doc;
  }

  captureSelection() {
    const selection = window.getSelection();
    const text = selection?.toString().trim();
    if (!text || !selection.anchorNode) return;
    const frame = selection.anchorNode.parentElement?.closest(".page");
    const page = frame ? this.viewer.frames.indexOf(frame) : -1;
    if (page < 0) return;
    this.selection = { text, page };
    if (this.active) this.showScope();
  }

  setScope(scope) {
    this.scope = scope;
    this.showScope();
    if (scope === "pages") byId("ai-pages").focus();
  }

  showScope() {
    for (const button of document.querySelectorAll("[data-scope]")) {
      const on = button.dataset.scope === this.scope;
      button.classList.toggle("on", on);
      button.classList.toggle("ghost", !on);
      button.setAttribute("aria-checked", String(on));
    }
    document.querySelector('[data-scope="selection"]').disabled = !this.selection;
    byId("ai-pages").hidden = this.scope !== "pages";
    const scope = this.request();
    byId("ai-scope-hint").textContent = scope ? this.describe(scope) : t("ai.scope.invalid");
  }

  /** Portée pour pde-app (signing… ai/mod.rs Scope), ou null si invalide. */
  request() {
    if (!this.doc) return null;
    const count = this.doc.pages.length;
    switch (this.scope) {
      case "selection":
        return this.selection ? { kind: "selection", ...this.selection } : null;
      case "page":
        return { kind: "pages", pages: [this.viewer.page] };
      case "pages": {
        const pages = parsePages(byId("ai-pages").value, count);
        return pages ? { kind: "pages", pages } : null;
      }
      default:
        return { kind: "document" };
    }
  }

  describe(scope) {
    if (scope.kind === "selection") return t("ai.scopeText.selection", { chars: scope.text.length, page: scope.page + 1 });
    if (scope.kind === "document") return t("ai.scopeText.document", { count: this.doc.pages.length });
    if (scope.pages.length === 1) return t("ai.scopeText.page", { page: scope.pages[0] + 1 });
    return t("ai.scopeText.pages", { count: scope.pages.length });
  }

  /** Estimation, confirmation, puis envoi. */
  async ask(task) {
    const scope = this.request();
    if (!this.doc || !scope) return toast(t("ai.scope.invalid"), "error");
    let estimate;
    try {
      estimate = await invoke("ai_estimate", { id: this.doc.id, scope, task });
    } catch (error) {
      return toast(t("ai.failed", { message: error?.message ?? String(error) }), "error");
    }
    if (estimate.over_cap) return toast(t("ai.overCap", { month: money(estimate.month_cost, estimate.currency) }), "error");
    if (estimate.needs_confirmation && !(await this.confirm(estimate, scope))) return;
    const entry = this.entry(task, scope);
    try {
      const run = await invoke("ai_start", { id: this.doc.id, scope, task, consent: true });
      entry.run = run;
      this.runs.set(run, entry);
      entry.cancel.onclick = () => invoke("ai_cancel", { run }).catch(() => {});
      if (task.kind === "question") byId("ai-question").value = "";
    } catch (error) {
      this.finish(entry, { Err: error });
    }
  }

  confirm(estimate, scope) {
    const dialog = byId("ai-confirm-dialog");
    byId("ai-confirm-text").textContent = t("ai.confirm.text", {
      host: estimate.host,
      scope: this.describe(scope),
      chars: estimate.characters.toLocaleString("fr-FR"),
    });
    byId("ai-confirm-cost").textContent = t("ai.confirm.cost", {
      requests: estimate.requests,
      cost: money(estimate.cost, estimate.currency),
    });
    return new Promise((resolve) => {
      const answer = (value) => () => {
        byId("ai-confirm-send").onclick = null;
        byId("ai-confirm-cancel").onclick = null;
        dialog.oncancel = null;
        if (dialog.open) dialog.close();
        resolve(value);
      };
      byId("ai-confirm-send").onclick = answer(true);
      byId("ai-confirm-cancel").onclick = answer(false);
      dialog.oncancel = answer(false);
      dialog.showModal();
      byId("ai-confirm-cancel").focus();
    });
  }

  /** Nouvelle réponse dans le panneau. */
  entry(task, scope) {
    const title = task.kind === "question" ? task.question : t(`ai.task.${task.kind}`);
    const body = el("div", { class: "ai-body" });
    const status = el("p", { class: "ai-status caption", role: "status" }, t("ai.waiting"));
    const cancel = el("button", { class: "btn ghost small", type: "button" }, t("ai.cancel"));
    const item = el(
      "li",
      { class: `ai-answer running ${task.kind === "question" ? "question" : "analysis"}` },
      el("div", { class: "ai-answer-head" }, el("strong", { class: "ai-answer-title" }, title), cancel),
      el("p", { class: "caption" }, this.describe(scope)),
      body,
      status,
    );
    byId("ai-answers").append(item);
    item.scrollIntoView({ block: "nearest" });
    return { item, body, status, cancel, text: "", run: null };
  }

  onEvent(payload) {
    const entry = this.runs.get(payload?.run);
    if (!entry) return;
    if (payload.kind === "reading") {
      entry.status.textContent = payload.done < payload.total ? t("ai.reading", { done: payload.done + 1, total: payload.total }) : t("ai.writing");
    } else if (payload.kind === "delta") {
      entry.text += payload.text;
      entry.status.textContent = t("ai.writing");
      this.render(entry);
    }
  }

  onDone(payload) {
    const entry = this.runs.get(payload?.run);
    if (!entry) return;
    this.runs.delete(payload.run);
    this.finish(entry, payload);
  }

  finish(entry, result) {
    entry.item.classList.remove("running");
    entry.cancel.remove();
    if (result.Ok) {
      entry.text = result.Ok.text;
      this.render(entry);
      const fallback = result.Ok.fallback_used ? ` · ${t("ai.fallbackUsed")}` : "";
      entry.status.textContent = `${result.Ok.model} · ${money(result.Ok.cost, result.Ok.currency)}${fallback}`;
      return;
    }
    const error = result.Err ?? {};
    entry.item.classList.add(error.kind === "cancelled" ? "cancelled" : "failed");
    entry.status.textContent = error.kind === "cancelled" ? t("ai.cancelled") : t("ai.failed", { message: error.message ?? "" });
  }

  /** Réponse en Markdown réduit, citations de pages cliquables. */
  render(entry) {
    const count = this.doc?.pages.length ?? 0;
    const nodes = [];
    let list = null;
    for (const block of blocks(entry.text, count)) {
      const parts = block.parts.map((part) => this.part(part));
      if (block.kind === "item") {
        list ??= nodes[nodes.push(el("ul", { class: "ai-list" })) - 1];
        list.append(el("li", {}, ...parts));
        continue;
      }
      list = null;
      nodes.push(el(block.kind === "heading" ? "h3" : "p", { class: `ai-${block.kind}` }, ...parts));
    }
    entry.body.replaceChildren(...nodes);
  }

  part(part) {
    if (part.kind === "bold") return el("strong", {}, part.text);
    if (part.kind !== "cite") return document.createTextNode(part.text);
    if (!part.valid) return el("span", { class: "ai-cite invalid", title: t("ai.citeInvalid") }, part.text);
    const button = el("button", { class: "ai-cite", type: "button", title: t("ai.citeGo", { page: part.pages[0] }) }, part.text);
    button.addEventListener("click", () => this.viewer.goToPage(part.pages[0] - 1));
    return button;
  }
}
