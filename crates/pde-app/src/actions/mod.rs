//! Catalogue des actions de l'éditeur : chaque action est décrite une seule
//! fois (nom, explication, mots-clés, outil dans la barre, raccourci,
//! paramètres en JSON Schema). La palette de commandes et l'aide « Comment
//! faire ? » en sont tirées ; l'assistant d'actions (IA-15) et le serveur MCP
//! (IA-17) le seront aussi : une action ajoutée ici est aussitôt connue de
//! tous.
//!
//! L'interface exécute les actions de la palette en actionnant leur outil ;
//! le contrôle et l'exécution par les cas d'usage, pour l'assistant, viendront
//! avec IA-15.

pub mod assistant;
mod list;

use pde_core::settings::Action as Shortcut;
use serde::Serialize;
use serde_json::Value;

/// Mode de l'éditeur où se trouve l'outil d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Read,
    Edit,
    Form,
    Pages,
    Sign,
    Ai,
}

/// Une action de l'éditeur.
#[derive(Debug, Clone, Serialize)]
pub struct ActionInfo {
    /// Identifiant stable (`snake_case`), aussi nom d'outil pour l'IA.
    pub id: &'static str,
    /// Rubrique de la palette (« Fichier », « Pages »…).
    pub group: &'static str,
    /// Nom affiché.
    pub title: &'static str,
    /// Explication en une ou deux phrases : ce que fait l'action, et où.
    pub help: &'static str,
    /// Autres mots par lesquels on la cherche (« Comment faire ? »).
    pub keywords: &'static [&'static str],
    /// Mode où se trouve l'outil (`None` : partout).
    pub mode: Option<Mode>,
    /// Élément de l'interface qui porte l'action (sélecteur CSS).
    pub tool: &'static str,
    /// Action possible seulement avec un document ouvert.
    pub needs_document: bool,
    /// Raccourci clavier réglable (R-08), s'il y en a un.
    pub shortcut: Option<Shortcut>,
    /// Paramètres en JSON Schema (objet ; vide si l'action n'en prend pas).
    pub params: Value,
}

/// Toutes les actions, dans l'ordre de la palette.
pub fn catalog() -> Vec<ActionInfo> {
    list::all()
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn ids_are_unique_and_usable_as_tool_names() {
        let actions = catalog();
        let mut seen = HashSet::new();
        for action in &actions {
            assert!(seen.insert(action.id), "{} en double", action.id);
            assert!(
                !action.id.is_empty()
                    && action.id.len() <= 64
                    && action
                        .id
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c == '_'),
                "identifiant « {} » invalide",
                action.id
            );
        }
    }

    #[test]
    fn every_action_is_explained_and_has_its_tool() {
        for action in catalog() {
            assert!(!action.title.is_empty() && !action.group.is_empty());
            assert!(
                action.help.len() > 20 && action.help.ends_with('.'),
                "{} : explication trop courte ou sans point final",
                action.id
            );
            assert!(
                action.tool.starts_with('#') || action.tool.starts_with('['),
                "{} : outil « {} »",
                action.id,
                action.tool
            );
            // Un outil d'un mode du document suppose un document ouvert.
            assert!(
                action.mode.is_none() || action.needs_document,
                "{}",
                action.id
            );
        }
    }

    #[test]
    fn params_are_object_schemas() {
        for action in catalog() {
            let schema = &action.params;
            assert_eq!(schema["type"], "object", "{}", action.id);
            assert_eq!(schema["additionalProperties"], false, "{}", action.id);
            let properties = schema["properties"]
                .as_object()
                .unwrap_or_else(|| panic!("{} : pas de « properties »", action.id));
            for property in properties.values() {
                assert!(
                    property["description"]
                        .as_str()
                        .is_some_and(|d| !d.is_empty()),
                    "{} : paramètre sans description",
                    action.id
                );
            }
            for required in schema["required"].as_array().into_iter().flatten() {
                let name = required.as_str().unwrap_or_default();
                assert!(properties.contains_key(name), "{} : {name}", action.id);
            }
        }
    }

    #[test]
    fn every_shortcut_runs_a_catalogued_action() {
        let catalogued: HashSet<_> = catalog().iter().filter_map(|a| a.shortcut).collect();
        for shortcut in Shortcut::ALL {
            // La palette et l'aide s'ouvrent elles-mêmes : pas d'entrée.
            if matches!(shortcut, Shortcut::Palette | Shortcut::HowTo) {
                continue;
            }
            assert!(catalogued.contains(&shortcut), "{shortcut:?} absent");
        }
    }

    /// Les tests de l'interface simulent le catalogue avec ce fichier : il
    /// doit rester identique à ce qu'envoie l'application.
    #[test]
    fn interface_test_fixture_matches_catalog() {
        // `PDE_UPDATE_EXPECTED=1` récrit le fichier après un changement du catalogue.
        if std::env::var_os("PDE_UPDATE_EXPECTED").is_some() {
            let path = concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../apps/desktop/ui-tests/fixtures/action-catalog.json"
            );
            let json = serde_json::to_string_pretty(&catalog()).unwrap();
            std::fs::write(path, json + "\n").unwrap();
            return;
        }
        let fixture =
            include_str!("../../../../apps/desktop/ui-tests/fixtures/action-catalog.json");
        let fixture: Value = serde_json::from_str(fixture).unwrap();
        assert_eq!(fixture, serde_json::to_value(catalog()).unwrap());
    }
}
