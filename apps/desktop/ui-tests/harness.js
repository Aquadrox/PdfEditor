// Banc de test de l'interface : sert apps/desktop/ui en HTTP local et
// l'ouvre dans Chromium avec un faux `window.__TAURI__`. Les réponses des
// commandes Rust sont décrites par chaque test (`commands`) ; chaque appel
// est enregistré dans `window.__calls` pour les vérifications.

import { createServer } from "node:http";
import { readFile } from "node:fs/promises";
import { extname, join, normalize } from "node:path";
import { fileURLToPath } from "node:url";

import { chromium } from "playwright";

const UI = fileURLToPath(new URL("../ui/", import.meta.url));
const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".woff2": "font/woff2",
};

function serve() {
  const server = createServer(async (request, response) => {
    const path = normalize(decodeURIComponent(new URL(request.url, "http://x").pathname));
    try {
      const body = await readFile(join(UI, path));
      response.writeHead(200, { "content-type": TYPES[extname(path)] ?? "application/octet-stream" });
      response.end(body);
    } catch {
      response.writeHead(404);
      response.end();
    }
  });
  return new Promise((resolve) => server.listen(0, "127.0.0.1", () => resolve(server)));
}

/**
 * Ouvre l'interface. `commands` : nom de commande → liste de réponses
 * successives ; une réponse `{ error: {...} }` est rejetée comme le ferait Tauri.
 */
export async function launch(commands = {}) {
  const server = await serve();
  const browser = await chromium.launch();
  const page = await browser.newPage({ viewport: { width: 1280, height: 820 } });
  const errors = [];
  page.on("pageerror", (error) => errors.push(error.message));
  await page.addInitScript((script) => {
    const queues = structuredClone(script);
    window.__calls = [];
    window.__TAURI__ = {
      core: {
        invoke: async (command, args) => {
          window.__calls.push({ command, args });
          const queue = queues[command] ?? [];
          const answer = queue.length > 1 ? queue.shift() : queue[0];
          if (answer && answer.error) throw answer.error;
          return answer?.ok ?? null;
        },
      },
      event: { listen: async () => () => {} },
      window: { getCurrentWindow: () => ({ setTitle: async () => {} }) },
    };
  }, commands);
  await page.goto(`http://127.0.0.1:${server.address().port}/index.html`);
  return {
    page,
    errors,
    calls: () => page.evaluate(() => window.__calls),
    async close() {
      await browser.close();
      server.close();
    },
  };
}

/** Document décrit comme le renvoie la commande open_document. */
export function documentView(pageCount = 3) {
  return {
    id: 1,
    file_name: "contrat.pdf",
    title: "Contrat",
    pages: Array.from({ length: pageCount }, () => ({ width: 595, height: 842 })),
    outline: [{ title: "Article 1", page: 0, children: [] }],
  };
}
