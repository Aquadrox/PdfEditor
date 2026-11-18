// Identités de signature (S-01, S-02) : liste, création d'un certificat
// personnel, import d'un fichier PKCS#12, suppression (en deux clics).

import { byId, el, fileName } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { formatTime, identityLabel } from "./sign-state.js";

const message = (error) => error?.message ?? String(error);

export class IdentityDialog {
  constructor() {
    this.dialog = byId("identity-dialog");
    this.tab = "create";
    this.file = null;
    this.created = null;
    this.resolve = null;
    byId("identity-tab-create").addEventListener("click", () => this.showTab("create"));
    byId("identity-tab-import").addEventListener("click", () => this.showTab("import"));
    byId("identity-pick").addEventListener("click", () => this.pick());
    byId("identity-close").addEventListener("click", () => this.dialog.close());
    byId("identity-form").addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });
    this.dialog.addEventListener("close", () => {
      const resolve = this.resolve;
      this.resolve = null;
      resolve?.(this.created);
    });
  }

  /** Ouvre le dialogue ; la promesse donne la dernière identité créée ou importée (ou null). */
  async open() {
    this.created = null;
    this.file = null;
    byId("identity-file").value = "";
    byId("identity-password").value = "";
    for (const id of ["identity-name", "identity-email", "identity-organization"]) byId(id).value = "";
    this.error(null);
    await this.refresh();
    this.showTab("create");
    this.dialog.showModal();
    byId("identity-name").focus();
    return new Promise((resolve) => {
      this.resolve = resolve;
    });
  }

  showTab(tab) {
    this.tab = tab;
    for (const [name, button] of [["create", byId("identity-tab-create")], ["import", byId("identity-tab-import")]]) {
      const on = name === tab;
      button.classList.toggle("on", on);
      button.classList.toggle("ghost", !on);
      button.setAttribute("aria-selected", String(on));
    }
    byId("identity-create").hidden = tab !== "create";
    byId("identity-import").hidden = tab !== "import";
    byId("identity-submit").textContent = t(tab === "create" ? "identity.submitCreate" : "identity.submitImport");
    this.error(null);
  }

  async refresh() {
    const identities = await invoke("identities").catch(() => []);
    const list = byId("identity-list");
    list.replaceChildren(
      ...(identities ?? []).map((identity) => {
        const remove = el("button", { class: "btn ghost danger small", type: "button" }, t("identity.delete"));
        remove.addEventListener("click", async () => {
          // Premier clic : on demande confirmation sur le même bouton.
          if (!remove.dataset.confirm) {
            remove.dataset.confirm = "1";
            remove.textContent = t("identity.confirmDelete");
            return;
          }
          try {
            await invoke("delete_identity", { id: identity.id });
            await this.refresh();
          } catch (error) {
            this.error(message(error));
          }
        });
        const until = t("identity.validUntil", { day: formatTime(identity.not_after).day });
        return el(
          "li",
          { class: "identity-item" },
          el("span", { class: "identity-text" }, el("strong", {}, identityLabel(identity)), el("span", { class: "caption" }, [identity.email, until].filter(Boolean).join(" · "))),
          remove,
        );
      }),
    );
  }

  async pick() {
    const path = await invoke("pick_certificate").catch((error) => {
      this.error(message(error));
      return null;
    });
    if (!path) return;
    this.file = path;
    byId("identity-file").value = fileName(path);
    byId("identity-password").focus();
  }

  async submit() {
    this.error(null);
    try {
      let identity;
      if (this.tab === "create") {
        const name = byId("identity-name").value.trim();
        if (!name) return this.error(t("identity.nameRequired"));
        const optional = (id) => byId(id).value.trim() || null;
        identity = await invoke("create_identity", {
          subject: { name, email: optional("identity-email"), organization: optional("identity-organization") },
        });
      } else {
        if (!this.file) return this.error(t("identity.fileRequired"));
        identity = await invoke("import_identity", { path: this.file, password: byId("identity-password").value });
      }
      this.created = identity;
      this.dialog.close();
    } catch (error) {
      this.error(error?.kind === "wrong_password" ? t("identity.wrongPassword") : message(error));
    }
  }

  error(text) {
    const box = byId("identity-error");
    box.hidden = !text;
    box.textContent = text ?? "";
  }
}
