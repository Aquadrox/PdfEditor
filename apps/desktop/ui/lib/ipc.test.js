import assert from "node:assert/strict";
import { test } from "node:test";

import { protocolBaseFor } from "./ipc.js";

test("adresse du protocole pde selon le WebView (Android compris)", () => {
  assert.equal(protocolBaseFor("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Edg/120"), "http://pde.localhost/");
  assert.equal(protocolBaseFor("Mozilla/5.0 (Linux; Android 14; 23078PND5G) AppleWebKit/537.36 Chrome/120 Mobile"), "http://pde.localhost/");
  assert.equal(protocolBaseFor("Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/605.1.15"), "pde://localhost/");
});
