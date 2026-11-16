# PdfEditor — notes pour Claude

- Le propriétaire ne code pas en Rust : Claude écrit tout le code. Expliquer les choix en termes Python/C/.NET si utile.
- Docs, commits et commentaires en français ; identifiants de code en anglais.
- Lire avant de coder : `docs/FONCTIONS.md` (identifiants E-01, S-05…), `docs/ARCHITECTURE.md` (couches, règles de dépendance), `docs/DESIGN.md` (interface).
- `docs/ROADMAP.md` est le suivi d'avancement : le mettre à jour à chaque étape livrée.
- Dev sur un serveur Debian sans Windows : tester sous Linux tout ce qui est portable, la CI Windows valide le reste. `pde-core` reste sans dépendance OS, PDFium, réseau ni Tauri.
- **Code propre** : un module = une responsabilité ; au-delà d'environ 400 lignes de code (hors tests), découper. Commandes Tauri sans logique (elles appellent `pde-app`). Pas de `unwrap()` hors des tests. `unsafe` seulement dans `pde-platform`, avec un commentaire `// SAFETY:`.
- **Tests dès le début** : chaque fonction livrée arrive avec ses tests ; chaque bogue corrigé avec le test qui le reproduit. Aucune vraie API IA en test (faux modèle `ChatModel`).
- PDFium (moteur PDF) : `./scripts/fetch-pdfium.sh` le télécharge dans `vendor/pdfium/` (jamais committé) ; les tests de `pde-pdf` en ont besoin.
- Avant de livrer : `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- **Git** : `main` est protégée ; tout passe par une PR dont la CI (`linux`, `windows`, `secrets`) est verte. Ne jamais pousser sur `main`, ne jamais mettre `[skip ci]`.
- **Version** : chaque PR qui change l'application monte `version` dans le Cargo.toml racine et ajoute en tête de `CHANGELOG.md` sa section `## X.Y.Z — AAAA-MM-JJ` (une ligne `- ` par changement, en mots d'utilisateur, `[Windows]` / `[Linux]` si propre à une plateforme).
- **Publication** : le propriétaire pose les tags `vX.Y.Z` ; ne jamais pousser de tag soi-même.
- **Reprise de VoiceToText** (dépôt `Aquadrox/VoiceToText`) : le code LLM, réglages, trousseau, mises à jour est copié puis adapté, voir le tableau de `docs/ARCHITECTURE.md`. Pas de dépendance entre les deux dépôts.
- **Dépôt public, aucun secret** : jamais de clé API, jeton, mot de passe, clé privée ou certificat (`.p12`, `.pfx`, `.pem`, `.key`), ni dans le code, les tests, les journaux collés ou les PR. Clés et certificats de test générés à l'exécution des tests. Les secrets de la CI passent par les secrets GitHub Actions. Vérifier le diff avant chaque push.
- **Données des utilisateurs** : jamais de contenu de document ni de prompt dans les journaux ; contenu des PDF affiché uniquement avec `textContent`.
