// Dialogue « Signer le document » : identité, apparence (signature
// manuscrite), motif, lieu, verrouillage, horodatage. Rend la demande de
// signature (pde-app signing/view.rs SignRequest), ou null si annulé.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { AppearancePicker } from "./appearance-picker.js";
import { appearanceLines, identityLabel } from "./sign-state.js";

/** Nom du serveur d'horodatage, pour la case à cocher. */
function hostOf(url) {
  return url.replace(/^https?:\/\//, "").split(/[/?]/)[0] || url;
}

/** Description de l'endroit signé, pour le dialogue. */
function where(target) {
  if (target.kind === "invisible") return t("sign.where.invisible");
  if (target.kind === "field") return t("sign.where.field", { name: target.name, page: target.pageNumber ?? "?" });
  return t("sign.where.new", { page: target.page + 1 });
}

export class SignDialog {
  /** @param {import("./identity-dialog.js").IdentityDialog} identities */
  constructor(identities) {
    this.dialog = byId("sign-dialog");
    this.identityDialog = identities;
    this.identities = [];
    this.lastIdentity = null;
    this.resolve = null;
    this.request = null;
    this.keepDrawing = null;
    this.appearance = new AppearancePicker();
    /** Rubrique Signature des réglages (R-05). */
    this.settings = { timestamp: false, timestamp_url: "", default_lock: "none" };
    byId("sign-cancel").addEventListener("click", () => this.dialog.close());
    byId("sign-new-identity").addEventListener("click", async () => {
      const created = await this.identityDialog.open();
      if (created) this.lastIdentity = created.id;
      await this.fillIdentities();
    });
    for (const radio of document.querySelectorAll('input[name="sign-lock"]')) {
      radio.addEventListener("change", () => this.updateSubmit());
    }
    byId("sign-identity-select").addEventListener("change", () => {
      this.appearance.setName(this.selected?.name);
      this.updateSubmit();
    });
    byId("sign-form").addEventListener("submit", (event) => {
      event.preventDefault();
      this.submit();
    });
    this.dialog.addEventListener("close", () => {
      const resolve = this.resolve;
      this.resolve = null;
      resolve?.(this.request);
    });
  }

  /**
   * Demande les réglages de la signature. `target` : cible (pde-app
   * TargetRequest, plus `pageNumber` pour un champ) ; `rules` : signingRules ;
   * `dirty` : modifications à enregistrer d'abord.
   */
  async ask(target, rules, dirty) {
    this.target = target;
    this.request = null;
    this.keepDrawing = null;
    byId("sign-where").textContent = where(target);
    byId("sign-unsaved").hidden = !dirty;
    byId("sign-error").hidden = true;
    const lock = rules.canLock ? this.settings.default_lock : "none";
    for (const radio of document.querySelectorAll('input[name="sign-lock"]')) {
      radio.checked = radio.value === lock;
      radio.disabled = radio.value !== "none" && !rules.canLock;
    }
    byId("sign-lock-note").hidden = rules.canLock;
    const server = this.settings.timestamp_url;
    byId("sign-timestamp").disabled = !server;
    byId("sign-timestamp").checked = Boolean(server) && this.settings.timestamp;
    byId("sign-timestamp-label").textContent = server ? t("sign.timestamp", { server: hostOf(server) }) : t("sign.timestampNone");
    await this.fillIdentities();
    await this.appearance.reset(Boolean(target.visible), this.selected?.name);
    this.dialog.showModal();
    (this.identities.length ? byId("sign-reason") : byId("sign-new-identity")).focus();
    return new Promise((resolve) => {
      this.resolve = resolve;
    });
  }

  async fillIdentities() {
    this.identities = (await invoke("identities").catch(() => [])) ?? [];
    const select = byId("sign-identity-select");
    select.replaceChildren(...this.identities.map((identity) => el("option", { value: identity.id }, identityLabel(identity))));
    if (this.identities.some((i) => i.id === this.lastIdentity)) select.value = this.lastIdentity;
    select.disabled = this.identities.length === 0;
    byId("sign-no-identity").hidden = this.identities.length > 0;
    this.updateSubmit();
  }

  get lock() {
    return document.querySelector('input[name="sign-lock"]:checked')?.value ?? "none";
  }

  updateSubmit() {
    byId("sign-submit-label").textContent = t(this.lock === "none" ? "sign.submit" : "sign.submitLock");
    byId("sign-submit").disabled = this.identities.length === 0;
  }

  get selected() {
    return this.identities.find((i) => i.id === byId("sign-identity-select").value) ?? null;
  }

  submit() {
    const identity = this.selected;
    if (!identity) return;
    const id = identity.id;
    let drawn;
    try {
      drawn = this.appearance.result();
    } catch (error) {
      const box = byId("sign-error");
      box.textContent = error.message;
      box.hidden = false;
      return;
    }
    this.lastIdentity = id;
    const { kind, name, page, x, y, width, height } = this.target;
    const target = kind === "field" ? { kind, name } : kind === "new" ? { kind, page, x, y, width, height } : { kind };
    const text = (input) => byId(input).value.trim() || null;
    this.request = {
      identity: id,
      target,
      reason: text("sign-reason"),
      location: text("sign-location"),
      lock: this.lock,
      appearance: appearanceLines(identity),
      image: drawn.image,
      timestamp: byId("sign-timestamp").checked,
    };
    // Gardée après une signature réussie (« Mes signatures »).
    this.keepDrawing = drawn.keep ? drawn.image : null;
    this.dialog.close();
  }

  /** Identité choisie en dernier (barre d'outils). */
  get chosen() {
    return this.identities.find((i) => i.id === this.lastIdentity) ?? this.identities[0] ?? null;
  }
}
