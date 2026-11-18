//! Recettes de l'assistant (IA-16) : intégrées (en lecture seule, livrées
//! avec l'application) et personnelles (un fichier TOML chacune dans
//! `recipes/` du dossier de données) ; import et export de fichiers pour
//! les partager. Une recette ne peut proposer que ses actions, toujours en
//! aperçu et sur accord.

use std::path::{Path, PathBuf};

use pde_core::recipe::Recipe;
use serde::Serialize;

use crate::actions::assistant::ASSISTANT_ACTIONS;
use crate::error::{AppError, ErrorKind};
use crate::files::write_atomic;

/// Recettes livrées avec l'application.
const BUILTIN: [(&str, &str); 3] = [
    ("rgpd", include_str!("rgpd.toml")),
    ("signature", include_str!("signature.toml")),
    ("montants", include_str!("amounts.toml")),
];

/// Préfixe des identifiants des recettes intégrées.
const BUILTIN_PREFIX: &str = "integree-";
/// Plus gros fichier de recette importé.
const MAX_FILE: u64 = 64 * 1024;
/// Nombre de recettes personnelles gardées.
pub const MAX_RECIPES: usize = 100;

fn invalid(message: impl Into<String>) -> AppError {
    AppError::new(ErrorKind::InvalidRequest, message)
}

fn io(error: std::io::Error) -> AppError {
    AppError::new(ErrorKind::Io, error.to_string())
}

/// Recette telle que l'interface la reçoit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RecipeView {
    pub id: String,
    pub builtin: bool,
    pub recipe: Recipe,
}

pub struct RecipeStore {
    dir: PathBuf,
}

/// Identifiant tiré d'un nom : minuscules sans accents, tirets.
fn slug(name: &str) -> String {
    let mut slug = String::new();
    for c in name.chars().flat_map(char::to_lowercase) {
        let c = match c {
            'à' | 'â' | 'ä' => 'a',
            'é' | 'è' | 'ê' | 'ë' => 'e',
            'î' | 'ï' => 'i',
            'ô' | 'ö' => 'o',
            'ù' | 'û' | 'ü' => 'u',
            'ç' => 'c',
            c => c,
        };
        if c.is_ascii_alphanumeric() {
            slug.push(c);
        } else if !slug.ends_with('-') && !slug.is_empty() {
            slug.push('-');
        }
    }
    let slug: String = slug.trim_end_matches('-').chars().take(48).collect();
    if slug.is_empty() {
        "recette".into()
    } else {
        slug
    }
}

/// Identifiant d'une recette personnelle : il nomme son fichier.
fn check_id(id: &str) -> Result<(), AppError> {
    let ok = !id.is_empty()
        && id.len() <= 64
        && !id.starts_with(BUILTIN_PREFIX)
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(invalid("recette inconnue"))
    }
}

impl RecipeStore {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("recipes"),
        }
    }

    fn builtins() -> impl Iterator<Item = RecipeView> {
        BUILTIN.into_iter().filter_map(|(id, text)| {
            Some(RecipeView {
                id: format!("{BUILTIN_PREFIX}{id}"),
                builtin: true,
                recipe: Recipe::parse(text, &ASSISTANT_ACTIONS).ok()?,
            })
        })
    }

    fn personal(&self) -> Vec<RecipeView> {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return Vec::new();
        };
        let mut recipes: Vec<RecipeView> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|e| e == "toml"))
            .filter_map(|path| {
                let id = path.file_stem()?.to_string_lossy().into_owned();
                check_id(&id).ok()?;
                let text = std::fs::read_to_string(&path).ok()?;
                match Recipe::parse(&text, &ASSISTANT_ACTIONS) {
                    Ok(recipe) => Some(RecipeView {
                        id,
                        builtin: false,
                        recipe,
                    }),
                    Err(error) => {
                        log::warn!("recette {} ignorée : {error}", path.display());
                        None
                    }
                }
            })
            .collect();
        recipes.sort_by_key(|r| r.recipe.name.to_lowercase());
        recipes.truncate(MAX_RECIPES);
        recipes
    }

    /// Recettes intégrées, puis personnelles par nom.
    pub fn list(&self) -> Vec<RecipeView> {
        Self::builtins().chain(self.personal()).collect()
    }

    pub fn get(&self, id: &str) -> Result<RecipeView, AppError> {
        self.list()
            .into_iter()
            .find(|r| r.id == id)
            .ok_or_else(|| invalid("recette inconnue"))
    }

    /// Enregistre une recette personnelle : `id` la remplace, sinon une
    /// nouvelle est créée (une recette intégrée ne se modifie pas : elle se
    /// duplique).
    pub fn save(&self, id: Option<&str>, recipe: Recipe) -> Result<RecipeView, AppError> {
        recipe.validate(&ASSISTANT_ACTIONS).map_err(invalid)?;
        let id = match id {
            Some(id) => {
                check_id(id)?;
                id.to_owned()
            }
            None => {
                if self.personal().len() >= MAX_RECIPES {
                    return Err(invalid(format!("{MAX_RECIPES} recettes au plus")));
                }
                self.free_id(&slug(&recipe.name))
            }
        };
        let text = recipe.to_toml().map_err(invalid)?;
        write_atomic(&self.dir.join(format!("{id}.toml")), text.as_bytes())?;
        Ok(RecipeView {
            id,
            builtin: false,
            recipe,
        })
    }

    /// `base`, ou `base-2`, `base-3`… s'il est déjà pris.
    fn free_id(&self, base: &str) -> String {
        let taken = |id: &str| self.dir.join(format!("{id}.toml")).exists();
        if !taken(base) {
            return base.to_owned();
        }
        (2..)
            .map(|n| format!("{base}-{n}"))
            .find(|id| !taken(id))
            .unwrap_or_else(|| base.to_owned())
    }

    pub fn delete(&self, id: &str) -> Result<(), AppError> {
        check_id(id)?;
        match std::fs::remove_file(self.dir.join(format!("{id}.toml"))) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(io(e)),
        }
    }

    /// Importe un fichier de recette (vérifié) comme nouvelle recette personnelle.
    pub fn import(&self, path: &Path) -> Result<RecipeView, AppError> {
        let size = std::fs::metadata(path).map_err(io)?.len();
        if size > MAX_FILE {
            return Err(invalid("fichier de recette trop gros"));
        }
        let text =
            std::fs::read_to_string(path).map_err(|_| invalid("fichier de recette illisible"))?;
        let recipe = Recipe::parse(&text, &ASSISTANT_ACTIONS).map_err(invalid)?;
        self.save(None, recipe)
    }

    /// Écrit la recette `id` (intégrée ou personnelle) dans `path`.
    pub fn export(&self, id: &str, path: &Path) -> Result<(), AppError> {
        let text = self.get(id)?.recipe.to_toml().map_err(invalid)?;
        write_atomic(path, text.as_bytes())
    }
}

#[cfg(test)]
mod tests;
