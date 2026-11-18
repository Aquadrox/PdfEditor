// Mode Signer (S-01 à S-07) : identités, signature dans un cadre tracé ou
// dans un champ vide, panneau Signatures, sceau, document verrouillé.

import assert from "node:assert/strict";
import { test } from "node:test";
import zlib from "node:zlib";

import { documentView, launch } from "./harness.js";

const EMPTY_EDIT = { elements: [], revisions: [0, 0, 0], layout: 0, fields: {}, metadata: null, flattened: false, can_undo: false, can_redo: false, dirty: false };
const IDENTITY = { id: "ab12", name: "Claire Martin", email: "claire@example.org", organization: "Atelier Morel", issuer: "Claire Martin", self_signed: true, origin: "self_signed", not_after: 1952683200, fingerprint: "AB12" };
const FIELD = { name: "Client", page: 0, bounds: { x: 300, y: 700, width: 200, height: 60 }, state: "empty", problem: null, signer: null, organization: null, issuer: null, self_signed: false, reason: null, location: null, time: null, modified_after: false, lock: null };
const SIGNED = { ...FIELD, name: "Signature1", bounds: null, state: "valid", signer: "Claire Martin", issuer: "Claire Martin", self_signed: true, time: 1795003200 };

async function openDocument(commands = {}) {
  const app = await launch({
    initial_files: [{ ok: ["/docs/contrat.pdf"] }],
    open_document: [{ ok: documentView(3) }],
    edit_view: [{ ok: EMPTY_EDIT }],
    form_fields: [{ ok: [] }],
    document_signatures: [{ ok: [] }],
    ...commands,
  });
  await app.page.locator("#doc-name", { hasText: "contrat.pdf" }).waitFor();
  await app.page.locator(".page img").first().waitFor({ state: "attached" });
  return app;
}

/** Vrai PNG 40 × 12 (RGBA) avec un trait d'encre, en base64. */
function inkPng() {
  const [width, height] = [40, 12];
  const rows = [];
  for (let y = 0; y < height; y++) {
    const row = Buffer.alloc(1 + width * 4);
    for (let x = 0; x < width; x++) if (y === 6) row.set([29, 42, 92, 255], 1 + x * 4);
    rows.push(row);
  }
  const chunk = (type, data) => {
    const body = Buffer.concat([Buffer.from(type), data]);
    const length = Buffer.alloc(4);
    length.writeUInt32BE(data.length);
    const crc = Buffer.alloc(4);
    crc.writeUInt32BE(zlib.crc32(body));
    return Buffer.concat([length, body, crc]);
  };
  const header = Buffer.alloc(13);
  header.writeUInt32BE(width, 0);
  header.writeUInt32BE(height, 4);
  header.set([8, 6, 0, 0, 0], 8);
  const png = Buffer.concat([
    Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]),
    chunk("IHDR", header),
    chunk("IDAT", zlib.deflateSync(Buffer.concat(rows))),
    chunk("IEND", Buffer.alloc(0)),
  ]);
  return png.toString("base64");
}

async function waitForCalls(app, command, count = 1) {
  await app.page.waitForFunction(([name, n]) => window.__calls.filter((c) => c.command === name).length >= n, [command, count]);
  return app.callsTo(command);
}

test("Signer : créer une identité puis signer dans un cadre tracé", async () => {
  const app = await openDocument({
    identities: [{ ok: [] }, { ok: [] }, { ok: [IDENTITY] }],
    create_identity: [{ ok: IDENTITY }],
    sign_document: [{ ok: { file_name: "contrat.pdf", path: "/docs/contrat.pdf" } }],
    document_signatures: [{ ok: [] }, { ok: [{ ...SIGNED, bounds: { x: 100, y: 100, width: 180, height: 54 } }] }],
  });
  const { page } = app;
  try {
    assert.equal(await page.locator("#seal").isHidden(), true);
    await page.click("#mode-sign");
    await page.locator("#sign-toolbar").waitFor();
    await page.locator("#signatures-panel").waitFor();
    assert.match(await page.locator("#sign-identity").textContent(), /Aucune identité/);

    // Cadre tracé sur la première page.
    await page.click("#sign-place");
    const layer = page.locator(".page").first().locator(".sign-layer.placing");
    await layer.waitFor();
    const box = await layer.boundingBox();
    await page.mouse.move(box.x + box.width * 0.2, box.y + box.height * 0.1);
    await page.mouse.down();
    await page.mouse.move(box.x + box.width * 0.5, box.y + box.height * 0.16, { steps: 5 });
    await page.mouse.up();

    // Dialogue : pas encore d'identité, on en crée une.
    await page.locator("#sign-dialog[open]").waitFor();
    assert.match(await page.locator("#sign-where").textContent(), /cadre tracé, page 1/);
    assert.equal(await page.locator("#sign-submit").isDisabled(), true);
    await page.click("#sign-new-identity");
    await page.locator("#identity-dialog[open]").waitFor();
    await page.click("#identity-submit");
    assert.match(await page.locator("#identity-error").textContent(), /nom/);
    await page.fill("#identity-name", "Claire Martin");
    await page.fill("#identity-organization", "Atelier Morel");
    await page.click("#identity-submit");
    await page.locator("#identity-dialog").waitFor({ state: "hidden" });
    assert.deepEqual((await app.callsTo("create_identity"))[0], { subject: { name: "Claire Martin", email: null, organization: "Atelier Morel" } });

    // La liste des identités est relue après la création : le bouton s'active alors.
    await page.locator("#sign-submit:not([disabled])").waitFor();
    assert.match(await page.locator("#sign-identity-select").textContent(), /Claire Martin/);
    await page.click('[data-appearance="text"]');
    await page.fill("#sign-reason", "Bon pour accord");
    await page.check('input[name="sign-lock"][value="forms"]');
    assert.equal(await page.locator("#sign-submit-label").textContent(), "Signer et verrouiller");
    await page.click("#sign-submit");

    const [call] = await waitForCalls(app, "sign_document");
    assert.equal(call.id, 1);
    assert.equal(call.request.identity, "ab12");
    assert.equal(call.request.lock, "forms");
    assert.equal(call.request.reason, "Bon pour accord");
    assert.equal(call.request.location, null);
    assert.equal(call.request.target.kind, "new");
    assert.equal(call.request.target.page, 0);
    assert.ok(call.request.target.width > 100 && call.request.target.height > 30, JSON.stringify(call.request.target));
    assert.equal(call.request.appearance[0], "Signé numériquement par Claire Martin");
    assert.equal(call.request.appearance[1], "Atelier Morel");

    // Le fichier signé est rouvert ; le sceau et le panneau le montrent.
    await waitForCalls(app, "open_document", 2);
    await page.locator("#seal:not([hidden])").waitFor();
    assert.equal(await page.locator("#seal-label").textContent(), "1 signature valide");
    await page.locator(".sig-item.valid").waitFor();
    assert.match(await page.locator(".sig-item.valid").textContent(), /Claire Martin.*Signature valide/);
    assert.match(await page.locator("#toast").textContent(), /signé et verrouillé/);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Signer : un champ vide se signe d'un clic, sans apparence en invisible", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    document_signatures: [{ ok: [FIELD] }],
    sign_document: [{ error: { kind: "signature", message: "le fichier a changé sur le disque" } }],
  });
  const { page } = app;
  try {
    assert.equal(await page.locator("#seal-label").textContent(), "À signer");
    await page.click("#seal");
    const field = page.locator(".page").first().locator(".sig-box.empty");
    await field.waitFor();
    assert.match(await page.locator("#sign-hint").textContent(), /champ en pointillés/);
    await field.click();
    await page.locator("#sign-dialog[open]").waitFor();
    assert.match(await page.locator("#sign-where").textContent(), /champ « Client », page 1/);
    assert.match(await page.locator("#sign-identity-select option").first().textContent(), /Claire Martin, certificat personnel/);
    await page.click('[data-appearance="text"]');
    await page.click("#sign-submit");
    const [call] = await waitForCalls(app, "sign_document");
    assert.deepEqual(call.request.target, { kind: "field", name: "Client" });
    assert.equal(call.request.lock, "none");
    // Échec : dit, et le document reste ouvert tel quel.
    await page.locator("#toast.error").waitFor();
    assert.match(await page.locator("#toast").textContent(), /a changé sur le disque/);
    assert.equal((await app.callsTo("open_document")).length, 1);

    // « Signer » : signature sans apparence ; Annuler n'envoie rien.
    await page.click("#sign-start");
    await page.locator("#sign-dialog[open]").waitFor();
    assert.match(await page.locator("#sign-where").textContent(), /sans apparence/);
    await page.click("#sign-cancel");
    await page.locator("#sign-dialog").waitFor({ state: "hidden" });
    assert.equal((await app.callsTo("sign_document")).length, 1);
  } finally {
    await app.close();
  }
});

test("Document signé : verrouiller n'est plus proposé, verrouillé : ni Éditer ni Pages", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    document_signatures: [{ ok: [{ ...SIGNED, lock: "all", reason: "Accord" }] }],
  });
  const { page } = app;
  try {
    await page.locator("#seal-label", { hasText: "Verrouillé" }).waitFor();
    assert.equal(await page.locator("#mode-edit").isDisabled(), true);
    assert.equal(await page.locator("#mode-pages").isDisabled(), true);
    await page.click("#mode-sign");
    assert.equal(await page.locator("#sign-start").isDisabled(), true);
    assert.match(await page.locator("#sign-hint").textContent(), /verrouillé/);
    const item = await page.locator(".sig-item").textContent();
    assert.match(item, /Motif : Accord/);
    assert.match(item, /Verrouille entièrement/);
    assert.match(item, /Certificat personnel/);
  } finally {
    await app.close();
  }
});

test("Document déjà signé : seule une signature d'approbation est possible", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    document_signatures: [{ ok: [SIGNED] }],
  });
  const { page } = app;
  try {
    await page.click("#mode-sign");
    await page.click("#sign-start");
    await page.locator("#sign-dialog[open]").waitFor();
    assert.equal(await page.locator('input[name="sign-lock"][value="forms"]').isDisabled(), true);
    assert.equal(await page.locator('input[name="sign-lock"][value="all"]').isDisabled(), true);
    assert.equal(await page.locator("#sign-lock-note").isVisible(), true);
    await page.click("#sign-cancel");
    // Passer à Éditer sur un document signé : prévenu une fois.
    await page.click("#mode-edit");
    assert.match(await page.locator("#toast").textContent(), /restent valides/);
  } finally {
    await app.close();
  }
});

test("Verrou « formulaires » : champs et signature seulement", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    document_signatures: [{ ok: [{ ...SIGNED, lock: "forms" }] }],
  });
  const { page } = app;
  try {
    await page.locator("#seal-label", { hasText: "1 signature valide" }).waitFor();
    assert.equal(await page.locator("#mode-edit").isDisabled(), true);
    assert.equal(await page.locator("#mode-pages").isDisabled(), true);
    assert.equal(await page.locator("#menu-properties").isDisabled(), true);
    assert.equal(await page.locator("#menu-flatten").isDisabled(), true);
    await page.click("#mode-sign");
    assert.equal(await page.locator("#sign-start").isDisabled(), false);
    assert.match(await page.locator("#sign-hint").textContent(), /seuls ses champs/);
  } finally {
    await app.close();
  }
});

test("Signature manuscrite dessinée, enregistrée, horodatée ; puis reprise de Mes signatures", async () => {
  const settings = structuredClone((await import("./harness.js")).DEFAULT_SETTINGS);
  settings.signature = { timestamp: true, timestamp_url: "http://tsa.example.org/tsr", default_lock: "forms" };
  const app = await openDocument({
    get_settings: [{ ok: { settings, warnings: [] } }],
    identities: [{ ok: [IDENTITY] }],
    signature_drawings: [{ ok: [] }],
    save_signature_drawing: [{ ok: { id: "aa11", png: "" } }],
    sign_document: [{ ok: { file_name: "contrat.pdf", path: "/docs/contrat.pdf" } }],
    document_signatures: [{ ok: [FIELD] }],
  });
  const { page } = app;
  try {
    await page.click("#mode-sign");
    await page.locator(".page").first().locator(".sig-box.empty").click();
    await page.locator("#sign-dialog[open]").waitFor();
    // Champ visible : la section Apparence est proposée, « Dessiner » d'abord.
    await page.locator("#sign-appearance:not([hidden])").waitFor();
    assert.equal(await page.locator('[data-appearance="draw"]').getAttribute("aria-pressed"), "true");
    // Réglages : verrouillage et horodatage proposés.
    assert.equal(await page.locator('input[name="sign-lock"][value="forms"]').isChecked(), true);
    assert.equal(await page.locator("#sign-timestamp").isChecked(), true);
    assert.match(await page.locator("#sign-timestamp-label").textContent(), /tsa\.example\.org/);

    // Zone vide : refusé avec un message.
    await page.click("#sign-submit");
    assert.match(await page.locator("#sign-error").textContent(), /Dessinez votre signature/);

    const pad = await page.locator("#sign-pad").boundingBox();
    await page.mouse.move(pad.x + 40, pad.y + pad.height / 2);
    await page.mouse.down();
    for (let i = 1; i <= 12; i++) await page.mouse.move(pad.x + 40 + i * 25, pad.y + pad.height / 2 + (i % 2 ? -20 : 20));
    await page.mouse.up();
    await page.check("#sign-keep");
    await page.click("#sign-submit");

    const [call] = await waitForCalls(app, "sign_document");
    assert.equal(call.request.timestamp, true);
    assert.equal(call.request.lock, "forms");
    assert.ok(call.request.image && call.request.image.length > 100, "PNG en base64");
    // Détourée : bien plus étroite que la zone de dessin.
    const png = Buffer.from(call.request.image, "base64");
    assert.equal(png.subarray(1, 4).toString(), "PNG");
    const width = png.readUInt32BE(16);
    assert.ok(width > 250 && width < 960, `largeur détourée : ${width}`);
    // Enregistrée dans Mes signatures après la signature.
    const [kept] = await waitForCalls(app, "save_signature_drawing");
    assert.equal(kept.png, call.request.image);
    assert.deepEqual(app.errors, []);
  } finally {
    await app.close();
  }
});

test("Signature tapée dans une police manuscrite, ou texte seul", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    signature_drawings: [{ ok: [] }],
    sign_document: [{ ok: { file_name: "contrat.pdf", path: "/docs/contrat.pdf" } }],
    document_signatures: [{ ok: [FIELD] }],
  });
  const { page } = app;
  try {
    await page.click("#mode-sign");
    await page.locator(".page").first().locator(".sig-box.empty").click();
    await page.locator("#sign-dialog[open]").waitFor();
    await page.click('[data-appearance="type"]');
    // Le nom de l'identité est proposé.
    assert.equal(await page.locator("#sign-typed").inputValue(), "Claire Martin");
    await page.selectOption("#sign-typed-font", "Dancing Script");
    await page.click("#sign-submit");
    let [call] = await waitForCalls(app, "sign_document");
    assert.ok(call.request.image, "nom tapé rendu en image");
    assert.equal(call.request.timestamp, false);
    assert.equal((await app.callsTo("save_signature_drawing")).length, 0);

    // Invisible : pas de section Apparence, pas d'image.
    await page.click("#sign-start");
    await page.locator("#sign-dialog[open]").waitFor();
    assert.equal(await page.locator("#sign-appearance").isHidden(), true);
    await page.click("#sign-submit");
    [, call] = await waitForCalls(app, "sign_document", 2);
    assert.equal(call.request.image, null);
  } finally {
    await app.close();
  }
});

test("Mes signatures : choisie d'un clic, supprimable", async () => {
  const tiny = inkPng();
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    signature_drawings: [{ ok: [{ id: "aa11", png: tiny }, { id: "bb22", png: tiny }] }],
    delete_signature_drawing: [{ ok: null }],
    sign_document: [{ ok: { file_name: "contrat.pdf", path: "/docs/contrat.pdf" } }],
    document_signatures: [{ ok: [FIELD] }],
  });
  const { page } = app;
  try {
    await page.click("#mode-sign");
    await page.locator(".page").first().locator(".sig-box.empty").click();
    await page.locator("#sign-dialog[open]").waitFor();
    // Signatures enregistrées : cet onglet d'abord, la première choisie.
    assert.equal(await page.locator('[data-appearance="saved"]').getAttribute("aria-pressed"), "true");
    await page.locator(".saved-drawing.on").waitFor();
    assert.equal(await page.locator("#sign-keep-row").isHidden(), true);
    await page.locator(".saved-item").nth(1).locator("button[aria-label*='Supprimer']").click();
    const [removed] = await waitForCalls(app, "delete_signature_drawing");
    assert.deepEqual(removed, { id: "bb22" });
    await page.click("#sign-submit");
    const [call] = await waitForCalls(app, "sign_document");
    assert.ok(call.request.image);
  } finally {
    await app.close();
  }
});

test("Modifié depuis la signature : ce qui a changé, et si le verrouillage l'interdisait", async () => {
  const app = await openDocument({
    identities: [{ ok: [IDENTITY] }],
    document_signatures: [
      {
        ok: [
          { ...SIGNED, lock: "forms", modified_after: true, changes: ["page_content", "form_fill"], changes_permitted: false },
          { ...SIGNED, name: "Signature2", signer: "Hugo Bernard", changes: [], changes_permitted: true },
        ],
      },
    ],
  });
  const { page } = app;
  try {
    await page.locator("#seal-label", { hasText: "Signature à vérifier" }).waitFor();
    await page.click("#mode-sign");
    const first = page.locator(".sig-item").first();
    assert.match(await first.textContent(), /Modifié depuis, ce que son verrouillage interdit : contenu des pages, remplissage du formulaire/);
    assert.match((await first.getAttribute("class")) ?? "", /concern-invalid/);
    assert.match(await page.locator(".sig-item").nth(1).textContent(), /Document inchangé depuis/);
  } finally {
    await app.close();
  }
});
