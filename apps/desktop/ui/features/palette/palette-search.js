// Recherche dans le catalogue des actions (palette de commandes et
// « Comment faire ? ») : sans accents ni majuscules, par début de mot, sur le
// nom, les mots-clés puis l'explication. Module pur, testé.

/** Mots d'une question qui ne désignent pas une action. */
const STOPWORDS = new Set(
  (
    "a au aux avec ce ces cet cette changer comment d de des du en est et faire fais fait faut il je j l " +
    "la le les ma me mes mettre mon ne on ou par peut peux pour puis qu que quel quelle sa se ses " +
    "son sur ta te tes ton tu un une veux voudrais y"
  ).split(" "),
);

/** Minuscules, sans accents ni ponctuation. */
export function normalize(text) {
  return text
    .normalize("NFD")
    .replace(/[\u0300-\u036f]/g, "")
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, " ")
    .trim();
}

/** Un pluriel simple compte comme le singulier. */
const singular = (word) => (word.length > 3 && word.endsWith("s") ? word.slice(0, -1) : word);

/** Mots d'un texte, au singulier. */
const wordsOf = (text) => normalize(text).split(" ").map(singular);

/** Mots utiles de la recherche. */
export function queryWords(query) {
  return normalize(query)
    .split(" ")
    .filter((word) => word && !STOPWORDS.has(word))
    .map(singular);
}

/** Poids d'un mot de la recherche dans un texte (0 : absent). */
function weight(word, words) {
  if (words.some((w) => w === word)) return 2;
  if (words.some((w) => w.startsWith(word))) return 1;
  return 0;
}

/** Pertinence d'une action : mots trouvés d'abord, puis où ils l'ont été. */
function score(action, words) {
  const fields = [
    [wordsOf(action.title), 6],
    [action.keywords.flatMap(wordsOf), 4],
    [wordsOf(`${action.group} ${action.id.replaceAll("_", " ")}`), 2],
    [wordsOf(action.help), 1],
  ];
  let found = 0;
  let total = 0;
  for (const word of words) {
    const best = Math.max(...fields.map(([texts, factor]) => weight(word, texts) * factor));
    if (best > 0) found += 1;
    total += best;
  }
  return { found, total };
}

/**
 * Actions qui répondent à `query`, les plus pertinentes d'abord ; toutes,
 * dans l'ordre du catalogue, si la recherche est vide.
 */
export function searchActions(actions, query) {
  const words = queryWords(query);
  if (words.length === 0) return [...actions];
  return actions
    .map((action, index) => ({ action, index, ...score(action, words) }))
    .filter((r) => r.found > 0)
    .sort((a, b) => b.found - a.found || b.total - a.total || a.index - b.index)
    .map((r) => r.action);
}
