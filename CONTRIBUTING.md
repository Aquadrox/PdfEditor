# Contribuer

## Règles

1. **Rien sur `main` sans PR.** Une PR = un sujet, référencé à la roadmap
   et aux fonctions (`E-03`, `S-05`…).
2. **Tests obligatoires.** Une fonction arrive avec ses tests ; un correctif
   avec le test qui reproduit le bogue (test de non-régression).
3. **CI verte** avant merge : jobs `linux`, `windows`, `secrets`.
4. **Code découpé** : un module = une responsabilité, environ 400 lignes de
   code maximum par fichier (hors tests).
5. **Aucun secret** dans le dépôt (voir plus bas).

Vérifications locales (une fois : `./scripts/fetch-pdfium.sh`, voir le README) :

```bash
cargo fmt --all
```

```bash
cargo clippy --workspace --all-targets -- -D warnings
```

```bash
cargo test --workspace
```

## Protection de `main` (réglage GitHub, à faire une fois)

Settings → Branches → *Add branch ruleset* (ou *Branch protection rule*) sur `main` :

- *Require a pull request before merging* (approbations : 0 si un seul mainteneur) ;
- *Require status checks to pass* : `linux`, `windows`, `secrets`, avec
  *Require branches to be up to date before merging* ;
- *Block force pushes* et interdiction de supprimer la branche ;
- *Do not allow bypassing the above settings* (s'applique aussi aux administrateurs).

Settings → Code security : activer *Secret scanning* et *Push protection*
(gratuits pour un dépôt public) en plus du job `secrets`.

## Secrets

- Clés API, jetons, mots de passe, clés privées et certificats ne vont
  **jamais** dans le dépôt, même dans un test ou un exemple.
- Les clés de l'utilisateur vivent dans le trousseau du système.
- Les tests génèrent leurs clés et certificats à l'exécution.
- La CI lit ses secrets dans les secrets GitHub Actions, et seulement dans
  les builds de tags (jamais dans les PR).
- Le `.gitignore` refuse `.env`, `*.key`, `*.pem`, `*.p12`, `*.pfx` ; le job
  `secrets` (gitleaks) analyse tout l'historique à chaque PR.
