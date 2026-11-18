import assert from "node:assert/strict";
import { test } from "node:test";

import {
  appearanceLines,
  details,
  documentLock,
  frameAt,
  frameBetween,
  identityLabel,
  sealLabel,
  signingRules,
  summary,
} from "./sign-state.js";

const valid = (extra = {}) => ({ name: "Signature1", page: 0, state: "valid", modified_after: false, lock: null, self_signed: true, time: 1795003200, ...extra });
const empty = (extra = {}) => ({ name: "Client", page: 3, state: "empty", lock: null, ...extra });

test("Sceau : aucun, valide, partiel, invalide, verrouillé", () => {
  assert.equal(summary([]).tone, "none");
  assert.deepEqual(sealLabel(summary([valid()])), ["seal.valid.one", { count: 1 }]);
  assert.deepEqual(sealLabel(summary([valid(), valid()])), ["seal.valid.many", { count: 2 }]);
  const partial = summary([valid(), empty()]);
  assert.equal(partial.tone, "warn");
  assert.deepEqual(sealLabel(partial), ["seal.partial", { signed: 1, total: 2 }]);
  assert.deepEqual(sealLabel(summary([empty()])), ["seal.toSign", { count: 1 }]);
  assert.equal(summary([valid(), { ...valid(), state: "invalid" }]).tone, "invalid");
  assert.deepEqual(sealLabel(summary([valid({ lock: "all" })])), ["seal.locked", {}]);
});

test("Signature suivie d'une modification : à vérifier, sauf ce que la certification attend", () => {
  assert.equal(summary([valid({ modified_after: true })]).tone, "warn");
  assert.equal(summary([valid({ modified_after: true, changes: ["page_content"] })]).tone, "warn");
  // Une seconde signature, ou des champs remplis sous un verrou « formulaires » : attendu.
  assert.equal(summary([valid({ modified_after: true, changes: ["signature"] }), valid()]).tone, "valid");
  assert.equal(summary([valid({ modified_after: true, lock: "forms", changes: ["form_fill", "signature"] }), valid()]).tone, "valid");
  // Interdit par le verrouillage : le sceau le dit.
  const forbidden = summary([valid({ modified_after: true, lock: "forms", changes: ["page_content"], changes_permitted: false })]);
  assert.equal(forbidden.tone, "invalid");
  assert.deepEqual(sealLabel(forbidden), ["seal.invalid", {}]);
});

test("Ce qui a changé depuis la signature, en mots", () => {
  const added = details(valid({ modified_after: true, changes: ["form_fill", "signature"] }));
  assert.deepEqual(added[1], ["sig.changes", { list: "remplissage du formulaire, nouvelle signature" }]);
  const forbidden = details(valid({ modified_after: true, changes: ["page_content"], changes_permitted: false }));
  assert.deepEqual(forbidden[1], ["sig.changesForbidden", { list: "contenu des pages" }]);
});

test("Verrouillage : seule une certification valide compte", () => {
  assert.equal(documentLock([valid({ lock: "forms" })]), "forms");
  assert.equal(documentLock([{ ...valid({ lock: "all" }), state: "invalid" }]), null);
  assert.deepEqual(signingRules([]), { canSign: true, canLock: true });
  assert.deepEqual(signingRules([empty()]), { canSign: true, canLock: true });
  assert.deepEqual(signingRules([valid()]), { canSign: true, canLock: false });
  assert.deepEqual(signingRules([valid({ lock: "all" })]), { canSign: false, canLock: false });
});

test("Détails du panneau : page, état, motif, certificat", () => {
  assert.deepEqual(details(empty()), [["sig.emptyOnPage", { page: 4 }]]);
  const lines = details(valid({ reason: "Accord", lock: "forms" }));
  assert.equal(lines[0][0], "sig.signedOnPage");
  assert.equal(lines[0][1].page, 1);
  assert.deepEqual(lines.slice(1), [["sig.unchanged", {}], ["sig.reason", { reason: "Accord" }], ["sig.lock.forms", {}], ["sig.selfSigned", {}]]);
  const broken = details({ ...valid(), state: "invalid", problem: "le document a été modifié après la signature" });
  assert.deepEqual(broken[1], ["sig.problem", { problem: "le document a été modifié après la signature" }]);
  assert.equal(broken.length, 2);
});

test("Horodatage : heure et serveur, ou raison du refus", () => {
  const stamped = details(valid({ timestamp: { time: 1795003200, authority: "DigiCert", problem: null } }));
  assert.equal(stamped[1][0], "sig.timestamp");
  assert.equal(stamped[1][1].authority, "DigiCert");
  const broken = details(valid({ timestamp: { time: null, authority: null, problem: "autre signature" } }));
  assert.deepEqual(broken[1], ["sig.timestampInvalid", { problem: "autre signature" }]);
});

test("Apparence : nom, organisation, date en français", () => {
  const lines = appearanceLines({ name: "Claire Martin", organization: "Atelier Morel" }, new Date(2026, 10, 18, 14, 32));
  assert.deepEqual(lines, ["Signé numériquement par Claire Martin", "Atelier Morel", "le 18/11/2026 à 14:32"]);
  assert.equal(appearanceLines({ name: "Hugo" }, new Date(2026, 0, 2, 9, 5)).length, 2);
});

test("Cadre : tracé borné à la page, clic simple centré sous la souris", () => {
  const page = { width: 595, height: 842 };
  assert.deepEqual(frameBetween({ x: 300, y: 800 }, { x: 100, y: 900 }, page), { x: 100, y: 800, width: 200, height: 42 });
  assert.equal(frameBetween({ x: 10, y: 10 }, { x: 20, y: 20 }, page), null);
  assert.deepEqual(frameAt({ x: 590, y: 5 }, page), { x: 415, y: 0, width: 180, height: 54 });
});

test("Identités : certificat personnel ou émis par une autorité", () => {
  assert.equal(identityLabel({ name: "Claire", self_signed: true }), "Claire, certificat personnel");
  assert.equal(identityLabel({ name: "Claire", self_signed: false, issuer: "Certigna" }), "Claire, certificat émis par Certigna");
});
