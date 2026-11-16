// Notes de version (Release de pde-core::changelog) : titre et liste, texte
// posé avec textContent (il peut venir du manifeste téléchargé).

import { el } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";

/** « Version 0.0.5 du 16 novembre 2026 », ou sans date si elle manque. */
export function releaseTitle(release) {
  return release.date
    ? t("updates.releaseDated", { version: release.version, date: release.date })
    : t("updates.release", { version: release.version });
}

export function renderReleases(container, releases) {
  container.replaceChildren(
    ...releases.map((release) =>
      el(
        "section",
        { class: "release" },
        el("h3", { class: "release-title" }, releaseTitle(release)),
        el("ul", {}, ...release.items.map((item) => el("li", {}, item))),
      ),
    ),
  );
}
