//! Recettes de l'assistant d'actions (IA-16) : une consigne enregistrée,
//! avec les seules actions qu'elle peut proposer et sa portée par défaut.
//! Format TOML, lisible et échangeable ; aucun code exécutable.

use serde::{Deserialize, Serialize};

/// Plus long nom de recette.
const MAX_NAME: usize = 80;
const MAX_DESCRIPTION: usize = 300;
const MAX_INSTRUCTION: usize = 4_000;

/// Portée proposée quand la recette est choisie.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecipeScope {
    Selection,
    Page,
    #[default]
    Document,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipe {
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Consigne donnée à l'assistant (ce qu'il doit chercher et proposer).
    pub instruction: String,
    /// Actions du catalogue que la recette peut proposer.
    pub actions: Vec<String>,
    #[serde(default)]
    pub scope: RecipeScope,
}

impl Recipe {
    /// Lit une recette TOML et la vérifie ; `known` : actions possibles.
    pub fn parse(text: &str, known: &[&str]) -> Result<Recipe, String> {
        let recipe: Recipe =
            toml::from_str(text).map_err(|e| format!("recette illisible : {}", e.message()))?;
        recipe.validate(known)?;
        Ok(recipe)
    }

    /// Recette telle qu'elle s'enregistre ou s'exporte.
    pub fn to_toml(&self) -> Result<String, String> {
        toml::to_string(self).map_err(|e| format!("recette non écrite : {e}"))
    }

    /// Nom, consigne et actions présents, dans les limites ; actions connues
    /// et sans doublon.
    pub fn validate(&self, known: &[&str]) -> Result<(), String> {
        let length = |text: &str, max: usize| text.trim().chars().count() <= max;
        if self.name.trim().is_empty() || !length(&self.name, MAX_NAME) {
            return Err(format!(
                "nom de recette vide ou de plus de {MAX_NAME} caractères"
            ));
        }
        if !length(&self.description, MAX_DESCRIPTION) {
            return Err(format!(
                "description de plus de {MAX_DESCRIPTION} caractères"
            ));
        }
        if self.instruction.trim().is_empty() || !length(&self.instruction, MAX_INSTRUCTION) {
            return Err(format!(
                "consigne vide ou de plus de {MAX_INSTRUCTION} caractères"
            ));
        }
        if self.actions.is_empty() {
            return Err("une recette permet au moins une action".into());
        }
        for (index, action) in self.actions.iter().enumerate() {
            if !known.contains(&action.as_str()) {
                return Err(format!("action « {action} » inconnue"));
            }
            if self.actions[..index].contains(action) {
                return Err(format!("action « {action} » en double"));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const KNOWN: [&str; 3] = ["redact", "highlight", "add_note"];

    const RGPD: &str = r#"
name = "Anonymiser"
description = "Données personnelles"
instruction = "Caviarde les noms, adresses et numéros de téléphone."
actions = ["redact"]
scope = "document"
"#;

    #[test]
    fn recipe_round_trips_through_toml() {
        let recipe = Recipe::parse(RGPD, &KNOWN).unwrap();
        assert_eq!(recipe.actions, ["redact"]);
        assert_eq!(recipe.scope, RecipeScope::Document);
        let again = Recipe::parse(&recipe.to_toml().unwrap(), &KNOWN).unwrap();
        assert_eq!(again, recipe);
    }

    #[test]
    fn scope_and_description_are_optional() {
        let recipe = Recipe::parse(
            "name = \"N\"\ninstruction = \"I\"\nactions = [\"add_note\"]",
            &KNOWN,
        )
        .unwrap();
        assert_eq!(recipe.scope, RecipeScope::Document);
        assert!(recipe.description.is_empty());
    }

    #[test]
    fn invalid_recipes_are_explained() {
        for (text, expected) in [
            ("name = ", "illisible"),
            (
                "name = \"\"\ninstruction = \"I\"\nactions = [\"redact\"]",
                "nom",
            ),
            (
                "name = \"N\"\ninstruction = \" \"\nactions = [\"redact\"]",
                "consigne",
            ),
            (
                "name = \"N\"\ninstruction = \"I\"\nactions = []",
                "au moins une action",
            ),
            (
                "name = \"N\"\ninstruction = \"I\"\nactions = [\"format_disk\"]",
                "inconnue",
            ),
            (
                "name = \"N\"\ninstruction = \"I\"\nactions = [\"redact\", \"redact\"]",
                "double",
            ),
            (
                "name = \"N\"\ninstruction = \"I\"\nactions = [\"redact\"]\nscope = \"tout\"",
                "illisible",
            ),
        ] {
            let error = Recipe::parse(text, &KNOWN).unwrap_err();
            assert!(error.contains(expected), "{text} → {error}");
        }
    }
}
