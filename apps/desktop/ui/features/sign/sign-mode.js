// Mode Signer (S-01 à S-07) : barre d'outils, cadres des champs sur les
// pages, panneau Signatures, sceau, et le parcours de signature : choisir
// où signer, régler la signature, enregistrer si besoin, signer, rouvrir
// le fichier signé.

import { toast } from "../../components/toast.js";
import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { IdentityDialog } from "./identity-dialog.js";
import { SignDialog } from "./sign-dialog.js";
import { renderSignLayer } from "./sign-layer.js";
import { pressSeal, renderPanel, renderSeal } from "./sign-panel.js";
import { identityLabel, signingRules, summary } from "./sign-state.js";

export class SignMode {
  /**
   * @param {import("../viewer/viewer.js").Viewer} viewer
   * @param {object} hooks
   * @param {import("../edit/edit-session.js").EditSession} hooks.session
   * @param {() => Promise<object|null>} hooks.save enregistre le document
   * @param {(path: string) => Promise<object|null>} hooks.reopen rouvre le fichier signé
   * @param {(info: object) => void} hooks.onState état des signatures (verrouillage)
   * @param {() => void} hooks.onSeal clic sur le sceau
   */
  constructor(viewer, hooks) {
    this.viewer = viewer;
    this.hooks = hooks;
    this.doc = null;
    this.signatures = [];
    this.active = false;
    this.placing = false;
    this.busy = false;
    this.warned = null;
    this.identities = new IdentityDialog();
    this.dialog = new SignDialog(this.identities);
    byId("sign-start").addEventListener("click", () => this.start({ kind: "invisible" }));
    byId("sign-place").addEventListener("click", () => this.setPlacing(!this.placing));
    byId("sign-identities").addEventListener("click", async () => {
      await this.identities.open();
      this.showIdentity();
    });
    byId("seal").addEventListener("click", () => hooks.onSeal());
  }

  /** Rubrique Signature des réglages : horodatage et verrouillage proposés. */
  applySettings(signature) {
    if (signature) this.dialog.settings = signature;
  }

  get info() {
    return summary(this.signatures);
  }

  /** Signatures du document ouvert (null : aucun document). */
  async load(doc) {
    this.doc = doc;
    this.signatures = [];
    this.placing = false;
    this.refresh();
    if (!doc) return;
    const signatures = await invoke("document_signatures", { id: doc.id }).catch(() => []);
    if (this.doc !== doc) return;
    this.signatures = signatures ?? [];
    this.refresh();
  }

  setActive(active) {
    this.active = active;
    if (!active) this.placing = false;
    byId("sign-toolbar").hidden = !active;
    byId("signatures-panel").hidden = !active;
    if (active) this.showIdentity();
    this.refresh();
  }

  setPlacing(placing) {
    this.placing = placing && this.active && signingRules(this.signatures).canSign;
    this.refresh();
  }

  refresh() {
    const info = this.info;
    renderSeal(info);
    renderPanel(this.signatures, { onField: (s) => this.signField(s), onPage: (page) => this.viewer.goToPage(page) });
    const { canSign } = signingRules(this.signatures);
    byId("sign-start").disabled = !this.doc || !canSign;
    byId("sign-place").disabled = !this.doc || !canSign;
    byId("sign-place").classList.toggle("on", this.placing);
    byId("sign-place").classList.toggle("ghost", !this.placing);
    byId("sign-place").setAttribute("aria-pressed", String(this.placing));
    const hasEmpty = this.signatures.some((s) => s.state === "empty" && s.bounds);
    byId("sign-hint").textContent = !canSign
      ? t("sign.lockedDocument")
      : info.lock === "forms"
        ? t("sign.formsOnly")
      : this.placing
        ? t("sign.placeActive")
        : hasEmpty
          ? t("sign.fieldHint")
          : "";
    this.viewer.frames.forEach((frame, index) => {
      if (frame.querySelector("img")) this.render(frame, index);
    });
    // Verrouillé par sa signature : ni Éditer, ni Pages, ni propriétés
    // (« formulaires » laisse remplir les champs et signer).
    for (const id of ["mode-edit", "mode-form", "mode-pages", "menu-properties"]) byId(id).disabled = !this.doc || Boolean(info.lock);
    this.hooks.onState(info);
  }

  /**
   * En passant à Éditer ou Pages sur un document signé : les modifications
   * seront ajoutées à la suite des signatures (F-03). Dit une fois par document.
   */
  warnIfSigned(doc) {
    if (!doc || this.warned === doc || this.info.signed === 0) return;
    this.warned = doc;
    toast(t("sign.signedWarning"));
  }

  /** Page affichée par la visionneuse. */
  pageShown(frame, index) {
    if (!frame.querySelector(".sign-layer")) this.render(frame, index);
  }

  render(frame, index) {
    frame.querySelector(".sign-layer")?.remove();
    if (!this.active || !this.viewer.doc) return;
    renderSignLayer(frame, index, this.viewer.doc.pages[index], {
      signatures: this.signatures,
      placing: this.placing,
      onField: (signature) => this.signField(signature),
      onFrame: (box) => {
        this.setPlacing(false);
        this.start({ kind: "new", page: index, visible: true, ...box });
      },
    });
  }

  async showIdentity() {
    const identities = (await invoke("identities").catch(() => [])) ?? [];
    const chosen = identities.find((i) => i.id === this.dialog.lastIdentity) ?? identities[0];
    byId("sign-identity").textContent = chosen ? t("sign.identityCurrent", { label: identityLabel(chosen) }) : t("sign.identityNone");
  }

  signField(signature) {
    return this.start({ kind: "field", name: signature.name, pageNumber: (signature.page ?? 0) + 1, visible: Boolean(signature.bounds) });
  }

  /** Parcours complet de la signature vers `target`. */
  async start(target) {
    if (!this.doc || this.busy) return;
    const rules = signingRules(this.signatures);
    if (!rules.canSign) return toast(t("sign.lockedDocument"), "error");
    const session = this.hooks.session;
    const request = await this.dialog.ask(target, rules, session.dirty);
    if (!request) return;
    this.busy = true;
    try {
      if (session.dirty && !(await this.hooks.save())) return;
      const saved = await invoke("sign_document", { id: this.doc.id, request });
      if (this.dialog.keepDrawing) {
        await invoke("save_signature_drawing", { png: this.dialog.keepDrawing }).catch((error) =>
          toast(t("sign.keepFailed", { message: error?.message ?? String(error) }), "error"),
        );
      }
      await this.hooks.reopen(saved.path);
      toast(t(request.lock === "none" ? "sign.done" : "sign.doneLocked"));
      pressSeal();
    } catch (error) {
      toast(t("sign.failed", { message: error?.message ?? String(error) }), "error");
    } finally {
      this.busy = false;
      this.showIdentity();
    }
  }
}
