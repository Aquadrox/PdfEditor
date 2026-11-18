// Apparence d'une signature visible (S-03) : texte seul, ou signature
// manuscrite dessinée, tapée (police manuscrite), importée (photo ou scan,
// papier rendu transparent) ou reprise de « Mes signatures ». Tout passe
// par un canevas ; le résultat est un PNG détouré, en base64.

import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke } from "../../lib/ipc.js";
import { clearPaper, contain, fittingSize, inkBounds } from "./pad-state.js";

const INK = "#1d2a5c";
const MODES = ["text", "draw", "type", "import", "saved"];

/** PNG en base64 → image dessinable. */
async function bitmapOf(base64) {
  const bytes = Uint8Array.from(atob(base64), (c) => c.charCodeAt(0));
  return createImageBitmap(new Blob([bytes], { type: "image/png" }));
}

export class AppearancePicker {
  constructor() {
    this.canvas = byId("sign-pad");
    this.context = this.canvas.getContext("2d", { willReadFrequently: true });
    this.mode = "draw";
    this.inked = false;
    this.saved = [];
    this.savedId = null;
    this.typedEdited = false;
    for (const button of document.querySelectorAll("[data-appearance]")) {
      button.addEventListener("click", () => this.setMode(button.dataset.appearance));
    }
    byId("sign-pad-clear").addEventListener("click", () => this.clear());
    byId("sign-typed").addEventListener("input", () => {
      this.typedEdited = true;
      this.renderTyped();
    });
    byId("sign-typed-font").addEventListener("change", () => this.renderTyped());
    byId("sign-import-file").addEventListener("change", () => this.renderImport());
    this.canvas.addEventListener("pointerdown", (event) => this.draw(event));
  }

  /** Nouveau dialogue : section montrée pour une signature visible. */
  async reset(visible, name) {
    byId("sign-appearance").hidden = !visible;
    this.visible = visible;
    this.typedEdited = false;
    byId("sign-typed").value = name ?? "";
    byId("sign-import-file").value = "";
    byId("sign-keep").checked = false;
    this.saved = visible ? ((await invoke("signature_drawings").catch(() => [])) ?? []) : [];
    this.savedId = null;
    this.setMode(this.saved.length > 0 ? "saved" : "draw");
  }

  /** Nom de l'identité choisie : repris par « Taper » s'il n'a pas été modifié. */
  setName(name) {
    if (this.typedEdited) return;
    byId("sign-typed").value = name ?? "";
    if (this.mode === "type") this.renderTyped();
  }

  setMode(mode) {
    this.mode = MODES.includes(mode) ? mode : "draw";
    for (const button of document.querySelectorAll("[data-appearance]")) {
      const on = button.dataset.appearance === this.mode;
      button.classList.toggle("on", on);
      button.classList.toggle("ghost", !on);
      button.setAttribute("aria-pressed", String(on));
    }
    byId("sign-pad-wrap").hidden = this.mode === "text";
    byId("sign-type-row").hidden = this.mode !== "type";
    byId("sign-import-row").hidden = this.mode !== "import";
    byId("sign-saved-list").hidden = this.mode !== "saved";
    byId("sign-keep-row").hidden = this.mode === "text" || this.mode === "saved";
    byId("sign-pad-clear").hidden = this.mode !== "draw";
    this.canvas.classList.toggle("drawing", this.mode === "draw");
    this.clear();
    if (this.mode === "type") this.renderTyped();
    if (this.mode === "saved") this.renderSaved();
  }

  clear() {
    this.context.clearRect(0, 0, this.canvas.width, this.canvas.height);
    this.inked = false;
  }

  draw(event) {
    if (this.mode !== "draw" || event.button !== 0) return;
    event.preventDefault();
    const rect = this.canvas.getBoundingClientRect();
    const point = (e) => [((e.clientX - rect.left) / rect.width) * this.canvas.width, ((e.clientY - rect.top) / rect.height) * this.canvas.height];
    const ctx = this.context;
    ctx.strokeStyle = INK;
    ctx.lineWidth = Math.max(3, this.canvas.width / 180);
    ctx.lineCap = "round";
    ctx.lineJoin = "round";
    let last = point(event);
    ctx.beginPath();
    ctx.arc(last[0], last[1], ctx.lineWidth / 2, 0, Math.PI * 2);
    ctx.fillStyle = INK;
    ctx.fill();
    this.inked = true;
    const move = (e) => {
      const next = point(e);
      ctx.beginPath();
      ctx.moveTo(...last);
      // Milieu du segment : tracé adouci.
      ctx.quadraticCurveTo(last[0], last[1], (last[0] + next[0]) / 2, (last[1] + next[1]) / 2);
      ctx.lineTo(...next);
      ctx.stroke();
      last = next;
    };
    const up = () => {
      this.canvas.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    this.canvas.setPointerCapture?.(event.pointerId);
    this.canvas.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  async renderTyped() {
    const text = byId("sign-typed").value.trim();
    const font = byId("sign-typed-font").value;
    await document.fonts.load(`80px "${font}"`, text || "a").catch(() => {});
    if (this.mode !== "type") return;
    this.clear();
    if (!text) return;
    const ctx = this.context;
    const size = fittingSize((s) => {
      ctx.font = `${s}px "${font}"`;
      return ctx.measureText(text).width;
    }, this.canvas.width * 0.92, Math.round(this.canvas.height * 0.6));
    ctx.font = `${size}px "${font}"`;
    ctx.fillStyle = INK;
    ctx.textAlign = "center";
    ctx.textBaseline = "middle";
    ctx.fillText(text, this.canvas.width / 2, this.canvas.height / 2);
    this.inked = true;
  }

  async renderImport() {
    const file = byId("sign-import-file").files?.[0];
    this.clear();
    if (!file) return;
    const bitmap = await createImageBitmap(file);
    const box = contain(bitmap.width, bitmap.height, this.canvas.width, this.canvas.height);
    this.context.drawImage(bitmap, box.x, box.y, box.width, box.height);
    const pixels = this.context.getImageData(0, 0, this.canvas.width, this.canvas.height);
    clearPaper(pixels.data);
    this.context.putImageData(pixels, 0, 0);
    this.inked = true;
  }

  async renderSaved() {
    const list = byId("sign-saved-list");
    if (this.saved.length === 0) {
      list.replaceChildren(el("p", { class: "caption" }, t("sign.appearance.noSaved")));
      return;
    }
    const items = await Promise.all(
      this.saved.map(async (drawing) => {
        const thumb = el("canvas", { width: 180, height: 56 });
        const bitmap = await bitmapOf(drawing.png).catch(() => null);
        if (bitmap) {
          const box = contain(bitmap.width, bitmap.height, 180, 56);
          thumb.getContext("2d").drawImage(bitmap, box.x, box.y, box.width, box.height);
        }
        const choose = el("button", { class: "saved-drawing", type: "button", "aria-label": t("sign.appearance.useSaved") }, thumb);
        choose.addEventListener("click", () => this.useSaved(drawing, bitmap, choose));
        const remove = el("button", { class: "btn ghost icon small", type: "button", "aria-label": t("sign.appearance.deleteSaved"), title: t("sign.appearance.deleteSaved") }, "×");
        remove.addEventListener("click", async () => {
          await invoke("delete_signature_drawing", { id: drawing.id }).catch(() => {});
          this.saved = this.saved.filter((d) => d.id !== drawing.id);
          if (this.savedId === drawing.id) this.clear();
          this.renderSaved();
        });
        return el("span", { class: "saved-item" }, choose, remove);
      }),
    );
    list.replaceChildren(...items);
    if (!this.savedId && this.saved[0]) items[0].querySelector(".saved-drawing").click();
  }

  useSaved(drawing, bitmap, button) {
    this.clear();
    this.savedId = drawing.id;
    for (const b of document.querySelectorAll(".saved-drawing")) b.classList.toggle("on", b === button);
    if (!bitmap) return;
    const box = contain(bitmap.width, bitmap.height, this.canvas.width, this.canvas.height);
    this.context.drawImage(bitmap, box.x, box.y, box.width, box.height);
    this.inked = true;
  }

  /**
   * Image choisie : `{ image, keep }` ; image null pour du texte seul.
   * Lève une erreur (message) si l'image attendue est vide.
   */
  result() {
    if (!this.visible || this.mode === "text") return { image: null, keep: false };
    const { width, height } = this.canvas;
    const pixels = this.context.getImageData(0, 0, width, height);
    const box = this.inked ? inkBounds(pixels.data, width, height) : null;
    if (!box) throw new Error(t(`sign.appearance.empty.${this.mode}`));
    const crop = el("canvas", { width: box.width, height: box.height });
    crop.getContext("2d").drawImage(this.canvas, box.x, box.y, box.width, box.height, 0, 0, box.width, box.height);
    const image = crop.toDataURL("image/png").split(",")[1];
    return { image, keep: this.mode !== "saved" && byId("sign-keep").checked };
  }
}
