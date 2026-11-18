// Mode Signer (S-01 à S-07) : identités, signature dans un cadre tracé ou
// dans un champ vide, panneau Signatures, sceau, document verrouillé.

import assert from "node:assert/strict";
import { test } from "node:test";

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

    assert.equal(await page.locator("#sign-submit").isDisabled(), false);
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
    assert.match(await page.locator("#toast").textContent(), /invalidera ses signatures/);
  } finally {
    await app.close();
  }
});
