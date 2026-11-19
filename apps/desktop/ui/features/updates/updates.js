// Mises à jour : bandeau quand une version est disponible, bandeau de
// nouveautés après une mise à jour, section des réglages. Rien n'est
// installé sans clic (voir apps/desktop/src/updates.rs ; Android :
// updates_mobile.rs, l'installateur du système prend ensuite le relais).

import { byId } from "../../lib/dom.js";
import { t } from "../../lib/i18n.js";
import { invoke, listen } from "../../lib/ipc.js";
import { renderReleases } from "./release-notes.js";

function toggle(button, panel) {
  button.addEventListener("click", () => {
    panel.hidden = !panel.hidden;
    button.setAttribute("aria-expanded", String(!panel.hidden));
  });
}

export function attachUpdates() {
  const install = byId("update-install");
  const settingsInstall = byId("settings-update-install");

  function showStatus(status) {
    const available = status.available;
    byId("update-banner").hidden = !available;
    settingsInstall.hidden = !available;
    if (available) {
      byId("update-text").textContent = t("updates.available", { version: available });
      settingsInstall.textContent = t("updates.installVersion", { version: available });
      renderReleases(byId("update-notes"), status.notes);
      byId("update-notes-toggle").hidden = status.notes.length === 0;
    }
    byId("update-check-status").textContent = !status.configured
      ? t("updates.notConfigured")
      : available
        ? t("updates.available", { version: available })
        : t("updates.upToDate", { version: status.current });
  }

  async function refresh() {
    showStatus(await invoke("update_status"));
  }

  let lastButton = install;
  // Android : suite déjà reçue du système (elle peut précéder la réponse).
  let androidState = null;

  function failed(message) {
    byId("update-text").textContent = t("updates.failed", { message });
    install.disabled = false;
    settingsInstall.disabled = false;
    lastButton.focus();
  }

  async function installNow(button) {
    lastButton = button;
    androidState = null;
    install.disabled = true;
    settingsInstall.disabled = true;
    byId("update-text").textContent = t("updates.downloading");
    try {
      // PC : l'application redémarre, la promesse ne se résout pas.
      // Android : le paquet vérifié est confié à l'installateur du système.
      await invoke("install_update");
      if (androidState === null) byId("update-text").textContent = t("updates.handedOver");
    } catch (error) {
      failed(String(error?.message ?? error));
    }
  }

  async function showNews() {
    const view = await invoke("changelog_view");
    renderReleases(byId("installed-notes"), view.releases);
    const shown = view.updated_from !== null && view.news.length > 0;
    byId("updated-banner").hidden = !shown;
    if (!shown) return;
    byId("updated-text").textContent = view.updated_from
      ? t("updates.updatedFrom", { version: view.current, from: view.updated_from })
      : t("updates.updated", { version: view.current });
    renderReleases(byId("updated-notes"), view.news);
  }

  toggle(byId("update-notes-toggle"), byId("update-notes"));
  toggle(byId("updated-notes-toggle"), byId("updated-notes"));
  install.addEventListener("click", () => installNow(install));
  settingsInstall.addEventListener("click", () => installNow(settingsInstall));
  byId("updated-close").addEventListener("click", () => {
    byId("updated-banner").hidden = true;
    invoke("dismiss_update_news").catch(() => {});
  });
  byId("update-check").addEventListener("click", async () => {
    byId("update-check").disabled = true;
    byId("update-check-status").textContent = t("updates.checking");
    try {
      showStatus(await invoke("check_update"));
    } catch (error) {
      byId("update-check-status").textContent = String(error?.message ?? error);
    } finally {
      byId("update-check").disabled = false;
    }
  });
  listen("update-available", () => refresh().catch(() => {}));
  listen("update-progress", (event) => {
    const percent = event.payload;
    byId("update-text").textContent =
      percent === null ? t("updates.downloading") : t("updates.progress", { percent });
  });

  // Android : suite de l'installation (fenêtre de confirmation, échec).
  listen("update-install", (event) => {
    const { state, message } = event.payload ?? {};
    androidState = state;
    if (state === "failed") failed(message || t("updates.unknownError"));
    else if (state === "confirm") byId("update-text").textContent = t("updates.confirm");
    else if (state === "installed") byId("update-text").textContent = t("updates.installed");
  });

  refresh().catch(() => {});
  showNews().catch(() => {});
}
