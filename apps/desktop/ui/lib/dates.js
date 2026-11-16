// Dates relatives en français (« il y a 5 min », « hier »). Module pur.

const MINUTE = 60;
const HOUR = 60 * MINUTE;

function sameDay(a, b) {
  return a.getFullYear() === b.getFullYear() && a.getMonth() === b.getMonth() && a.getDate() === b.getDate();
}

/** `seconds` et `nowSeconds` : secondes depuis le 1er janvier 1970. */
export function relativeDate(seconds, nowSeconds) {
  const elapsed = nowSeconds - seconds;
  if (elapsed < MINUTE) return "à l'instant";
  if (elapsed < HOUR) return `il y a ${Math.floor(elapsed / MINUTE)} min`;
  const date = new Date(seconds * 1000);
  const now = new Date(nowSeconds * 1000);
  const time = date.toLocaleTimeString("fr-FR", { hour: "2-digit", minute: "2-digit" });
  if (sameDay(date, now)) return `aujourd'hui à ${time}`;
  const yesterday = new Date(now);
  yesterday.setDate(now.getDate() - 1);
  if (sameDay(date, yesterday)) return `hier à ${time}`;
  const options = { day: "numeric", month: "long" };
  if (date.getFullYear() !== now.getFullYear()) options.year = "numeric";
  return date.toLocaleDateString("fr-FR", options);
}
