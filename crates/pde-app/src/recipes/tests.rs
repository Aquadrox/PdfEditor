use pde_core::recipe::RecipeScope;

use super::*;

fn store(name: &str) -> (RecipeStore, PathBuf) {
    let dir = std::env::temp_dir().join(format!("pde-recipes-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    (RecipeStore::new(&dir), dir)
}

fn recipe(name: &str) -> Recipe {
    Recipe {
        name: name.into(),
        description: String::new(),
        instruction: "Surligne les dates.".into(),
        actions: vec!["highlight".into()],
        scope: RecipeScope::Page,
    }
}

#[test]
fn builtin_recipes_are_valid_and_read_only() {
    let (store, dir) = store("builtin");
    let list = store.list();
    assert_eq!(list.len(), BUILTIN.len(), "chaque recette intégrée se lit");
    assert!(
        list.iter()
            .all(|r| r.builtin && r.id.starts_with(BUILTIN_PREFIX))
    );
    let rgpd = store.get("integree-rgpd").unwrap();
    assert_eq!(rgpd.recipe.actions, ["redact"]);
    // Ni modifiée ni supprimée : elle se duplique.
    assert!(store.save(Some("integree-rgpd"), recipe("x")).is_err());
    assert!(store.delete("integree-rgpd").is_err());
    let copy = store.save(None, rgpd.recipe.clone()).unwrap();
    assert_eq!(copy.id, "anonymiser-selon-le-rgpd");
    assert!(!copy.builtin);
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn personal_recipes_are_saved_renamed_and_deleted() {
    let (store, dir) = store("personal");
    let first = store.save(None, recipe("Dates à vérifier")).unwrap();
    assert_eq!(first.id, "dates-a-verifier");
    let second = store.save(None, recipe("Dates à vérifier")).unwrap();
    assert_eq!(
        second.id, "dates-a-verifier-2",
        "même nom : identifiant distinct"
    );
    let mut changed = recipe("Dates (révisé)");
    changed.actions = vec!["highlight".into(), "add_note".into()];
    store.save(Some(&first.id), changed.clone()).unwrap();
    assert_eq!(store.get(&first.id).unwrap().recipe, changed);
    store.delete(&second.id).unwrap();
    let names: Vec<String> = store
        .list()
        .into_iter()
        .filter(|r| !r.builtin)
        .map(|r| r.recipe.name)
        .collect();
    assert_eq!(names, ["Dates (révisé)"]);
    // Identifiants qui sortiraient du dossier, ou recette invalide : refusés.
    assert!(store.delete("../config").is_err());
    assert!(store.save(Some("../x"), recipe("x")).is_err());
    let mut empty = recipe("Sans action");
    empty.actions.clear();
    assert!(store.save(None, empty).is_err());
    let _ = std::fs::remove_dir_all(dir);
}

#[test]
fn recipes_are_exported_and_imported_as_files() {
    let (store, dir) = store("files");
    let file = dir.join("partage.toml");
    store.export("integree-montants", &file).unwrap();
    let imported = store.import(&file).unwrap();
    assert_eq!(imported.recipe.name, "Relire les montants");
    assert!(!imported.builtin);

    // Fichier invalide : refusé avec sa raison ; illisible dans le dossier : ignoré.
    std::fs::write(
        &file,
        "name = \"X\"\ninstruction = \"I\"\nactions = [\"supprime_tout\"]",
    )
    .unwrap();
    assert!(
        store
            .import(&file)
            .unwrap_err()
            .message
            .contains("inconnue")
    );
    std::fs::write(dir.join("recipes/casse.toml"), "pas une recette").unwrap();
    assert_eq!(store.list().iter().filter(|r| !r.builtin).count(), 1);
    let _ = std::fs::remove_dir_all(dir);
}
