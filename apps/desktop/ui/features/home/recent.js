// Documents récents sur l'accueil.

import { relativeDate } from "../../lib/dates.js";
import { byId, el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

/** Affiche la liste ; un clic ouvre le document. Liste vide : rien n'est affiché. */
export function renderRecent(files, onOpen) {
  const section = byId("recent");
  section.hidden = files.length === 0;
  const now = Date.now() / 1000;
  byId("recent-list").replaceChildren(
    ...files.map((file) =>
      el(
        "li",
        {},
        el(
          "button",
          {
            class: "recent",
            disabled: !file.exists,
            title: file.exists ? file.path : t("recent.missing"),
            onclick: () => onOpen(file.path),
          },
          el("span", { class: "recent-name" }, file.name),
          el("span", { class: "recent-date" }, relativeDate(file.opened_at, now)),
          el("span", { class: "recent-folder" }, file.folder),
        ),
      ),
    ),
  );
}
