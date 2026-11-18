//! Raccourcis clavier (R-08) : notation « Ctrl+Maj+F », vérification des
//! doublons. La notation est celle que l'interface compare aux touches.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Actions qui ont un raccourci. L'ordre est celui de la page Réglages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Open,
    Close,
    Search,
    ZoomIn,
    ZoomOut,
    ZoomReset,
    FitPage,
    FitWidth,
    Settings,
    Undo,
    Redo,
    Save,
    SaveAs,
    Palette,
    HowTo,
}

impl Action {
    pub const ALL: [Action; 15] = [
        Action::Open,
        Action::Close,
        Action::Search,
        Action::ZoomIn,
        Action::ZoomOut,
        Action::ZoomReset,
        Action::FitPage,
        Action::FitWidth,
        Action::Settings,
        Action::Undo,
        Action::Redo,
        Action::Save,
        Action::SaveAs,
        Action::Palette,
        Action::HowTo,
    ];

    pub fn default_shortcut(self) -> &'static str {
        match self {
            Action::Open => "Ctrl+O",
            Action::Close => "Ctrl+W",
            Action::Search => "Ctrl+F",
            Action::ZoomIn => "Ctrl+Plus",
            Action::ZoomOut => "Ctrl+Minus",
            Action::ZoomReset => "Ctrl+0",
            Action::FitPage => "Ctrl+1",
            Action::FitWidth => "Ctrl+2",
            Action::Settings => "Ctrl+Comma",
            Action::Undo => "Ctrl+Z",
            Action::Redo => "Ctrl+Y",
            Action::Save => "Ctrl+S",
            Action::SaveAs => "Ctrl+Shift+S",
            Action::Palette => "Ctrl+K",
            Action::HowTo => "F1",
        }
    }
}

/// Modificateurs, dans l'ordre d'écriture.
const MODIFIERS: [&str; 3] = ["Ctrl", "Alt", "Shift"];
/// Touches nommées acceptées (en plus des lettres, chiffres et F1 à F12).
const NAMED_KEYS: [&str; 12] = [
    "Plus", "Minus", "Comma", "Period", "Space", "Enter", "Tab", "Home", "End", "PageUp",
    "PageDown", "Delete",
];

/// Écrit un raccourci sous sa forme normale (« ctrl + shift + f » →
/// « Ctrl+Shift+F »), ou explique pourquoi il est refusé.
pub fn normalize(shortcut: &str) -> Result<String, String> {
    let parts: Vec<&str> = shortcut.split('+').map(str::trim).collect();
    let (key, modifiers) = parts
        .split_last()
        .filter(|(key, _)| !key.is_empty())
        .ok_or_else(|| format!("raccourci vide ou incomplet : « {shortcut} »"))?;
    let mut found = Vec::new();
    for modifier in modifiers {
        let canonical = MODIFIERS
            .iter()
            .find(|m| m.eq_ignore_ascii_case(modifier))
            .ok_or_else(|| format!("modificateur inconnu « {modifier} » dans « {shortcut} »"))?;
        if !found.contains(canonical) {
            found.push(*canonical);
        }
    }
    found.sort_by_key(|m| MODIFIERS.iter().position(|x| x == m));
    let key = normalize_key(key)
        .ok_or_else(|| format!("touche inconnue « {key} » dans « {shortcut} »"))?;
    let function_key = key.len() > 1 && key.starts_with('F') && key[1..].parse::<u8>().is_ok();
    // Sans Ctrl ni Alt, une lettre serait tapée dans les champs de saisie.
    if !function_key && !found.iter().any(|m| *m == "Ctrl" || *m == "Alt") {
        return Err(format!(
            "« {shortcut} » : Ctrl ou Alt nécessaire (sauf touches F1 à F12)"
        ));
    }
    found.push(&key);
    Ok(found.join("+"))
}

fn normalize_key(key: &str) -> Option<String> {
    let mut chars = key.chars();
    if let (Some(c), None) = (chars.next(), chars.next())
        && c.is_ascii_alphanumeric()
    {
        return Some(c.to_ascii_uppercase().to_string());
    }
    if let Some(n) = key
        .strip_prefix(['F', 'f'])
        .and_then(|n| n.parse::<u8>().ok())
        && (1..=12).contains(&n)
    {
        return Some(format!("F{n}"));
    }
    NAMED_KEYS
        .iter()
        .find(|k| k.eq_ignore_ascii_case(key))
        .map(|k| (*k).to_owned())
}

/// Raccourcis de toutes les actions.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Shortcuts(pub BTreeMap<Action, String>);

impl Default for Shortcuts {
    fn default() -> Self {
        Self(
            Action::ALL
                .iter()
                .map(|a| (*a, a.default_shortcut().to_owned()))
                .collect(),
        )
    }
}

impl Shortcuts {
    /// Normalise chaque raccourci ; une action absente ou invalide reprend
    /// son raccourci par défaut (avec un avertissement). Deux actions sur la
    /// même touche : erreur, l'utilisateur doit choisir.
    pub fn validated(&self, warnings: &mut Vec<String>) -> Result<Shortcuts, String> {
        let mut result = BTreeMap::new();
        for action in Action::ALL {
            let shortcut = match self.0.get(&action).map(|s| normalize(s)) {
                Some(Ok(shortcut)) => shortcut,
                Some(Err(e)) => {
                    warnings.push(format!("{e} : raccourci par défaut rétabli"));
                    action.default_shortcut().to_owned()
                }
                None => action.default_shortcut().to_owned(),
            };
            if let Some((other, _)) = result.iter().find(|(_, s)| **s == shortcut) {
                return Err(format!(
                    "« {shortcut} » est attribué à deux actions ({other:?} et {action:?})"
                ));
            }
            result.insert(action, shortcut);
        }
        Ok(Shortcuts(result))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcuts_are_normalized() {
        assert_eq!(normalize("ctrl + shift + f").unwrap(), "Ctrl+Shift+F");
        assert_eq!(normalize("Shift+Ctrl+F").unwrap(), "Ctrl+Shift+F");
        assert_eq!(normalize("Ctrl+plus").unwrap(), "Ctrl+Plus");
        assert_eq!(normalize("f3").unwrap(), "F3");
        assert_eq!(normalize("Alt+pageup").unwrap(), "Alt+PageUp");
    }

    #[test]
    fn invalid_shortcuts_are_explained() {
        for bad in ["", "Ctrl+", "Hyper+F", "Ctrl+Truc", "F", "Shift+A", "F13"] {
            assert!(normalize(bad).is_err(), "« {bad} » accepté");
        }
    }

    #[test]
    fn defaults_are_valid_and_distinct() {
        let mut warnings = Vec::new();
        let shortcuts = Shortcuts::default().validated(&mut warnings).unwrap();
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(shortcuts.0.len(), Action::ALL.len());
    }

    #[test]
    fn invalid_or_missing_entries_fall_back_to_defaults() {
        let mut map = BTreeMap::new();
        map.insert(Action::Open, "Ctrl+Truc".to_owned());
        map.insert(Action::Search, "f3".to_owned());
        let mut warnings = Vec::new();
        let shortcuts = Shortcuts(map).validated(&mut warnings).unwrap();
        assert_eq!(shortcuts.0[&Action::Open], "Ctrl+O");
        assert_eq!(shortcuts.0[&Action::Search], "F3");
        assert_eq!(shortcuts.0[&Action::Close], "Ctrl+W");
        assert_eq!(warnings.len(), 1);
    }

    #[test]
    fn duplicates_are_refused() {
        let mut map = Shortcuts::default().0;
        map.insert(Action::Search, "ctrl+o".to_owned());
        let error = Shortcuts(map).validated(&mut Vec::new()).unwrap_err();
        assert!(error.contains("Ctrl+O"), "{error}");
    }
}
