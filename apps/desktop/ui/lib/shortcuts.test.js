import assert from "node:assert/strict";
import { test } from "node:test";

import { eventToShortcut, isAcceptable, keymap, shortcutLabel } from "./shortcuts.js";

const key = (key, extra = {}) => ({ key, code: "", ctrlKey: false, altKey: false, shiftKey: false, metaKey: false, ...extra });

test("lettres, chiffres et touches de fonction", () => {
  assert.equal(eventToShortcut(key("o", { ctrlKey: true })), "Ctrl+O");
  assert.equal(eventToShortcut(key("F", { ctrlKey: true, shiftKey: true })), "Ctrl+Shift+F");
  assert.equal(eventToShortcut(key("F3")), "F3");
  assert.equal(eventToShortcut(key("Home", { altKey: true })), "Alt+Home");
});

test("chiffres selon la touche physique (clavier AZERTY)", () => {
  assert.equal(eventToShortcut(key("à", { ctrlKey: true, code: "Digit0" })), "Ctrl+0");
  assert.equal(eventToShortcut(key("&", { ctrlKey: true, code: "Digit1" })), "Ctrl+1");
});

test("plus et moins, avec ou sans Maj", () => {
  assert.equal(eventToShortcut(key("=", { ctrlKey: true })), "Ctrl+Plus");
  assert.equal(eventToShortcut(key("+", { ctrlKey: true, shiftKey: true })), "Ctrl+Plus");
  assert.equal(eventToShortcut(key("-", { ctrlKey: true })), "Ctrl+Minus");
  assert.equal(eventToShortcut(key(",", { ctrlKey: true })), "Ctrl+Comma");
});

test("modificateur seul ou touche inconnue : pas de raccourci", () => {
  assert.equal(eventToShortcut(key("Control", { ctrlKey: true })), null);
  assert.equal(eventToShortcut(key("Dead", { ctrlKey: true })), null);
});

test("mêmes règles que pde-core", () => {
  assert.equal(isAcceptable("Ctrl+O"), true);
  assert.equal(isAcceptable("F3"), true);
  assert.equal(isAcceptable("Shift+A"), false);
  assert.equal(isAcceptable("O"), false);
});

test("affichage en français", () => {
  assert.equal(shortcutLabel("Ctrl+Shift+F"), "Ctrl+Maj+F");
  assert.equal(shortcutLabel("Ctrl+Plus"), "Ctrl++");
  assert.equal(shortcutLabel("Ctrl+Comma"), "Ctrl+,");
});

test("table des actions", () => {
  assert.equal(keymap({ open: "Ctrl+O", search: "F3" }).get("F3"), "search");
});
