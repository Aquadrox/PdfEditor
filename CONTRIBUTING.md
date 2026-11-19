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

Après un ajout ou une mise à jour de dépendance (vulnérabilités connues,
licences compatibles, voir `deny.toml`) :

```bash
cargo deny check
```

```bash
(cd apps/desktop/ui && node --test) && (cd apps/desktop/ui-tests && npm ci && npm test)
```

## Titres des PR et des commits

Format (comme VoiceToText, vérifié sur chaque PR par `.github/workflows/titre-pr.yml`) :

```text
type(portée): description en français (vX.Y.Z)
```

- **type** :
  - `feat` : nouvelle fonction pour l'utilisateur ;
  - `fix` : correction de bogue ;
  - `perf` : plus rapide ou plus léger ;
  - `refactor` : réorganisation du code sans changement visible ;
  - `test` : tests seulement ;
  - `docs` : documentation (roadmap, architecture, design…) ;
  - `ci` : intégration continue, publication ;
  - `build` : dépendances, compilation, installeur ;
  - `chore` : entretien divers (outillage, scripts).
- **portée** (facultative, en minuscules) : la partie touchée, par exemple
  `core`, `pdf`, `app`, `platform`, `desktop`, `ui`, `sign`, `ia`, `cycle`,
  `android`, `roadmap`, `deps`. Plusieurs portées : `(core,ui)`.
- **description** : en français, au présent, sans point final, ce que la PR
  apporte vu de l'utilisateur quand c'est possible.
- **(vX.Y.Z)** : à la fin, seulement si la PR monte la version.

Exemples :

```text
feat(ui): recherche dans le document (v0.0.3)
fix(desktop): Entrée validait « Annuler » dans le dialogue du mot de passe (v0.0.6)
ci: contrôle des dépendances par cargo-deny
docs(roadmap): compatibilité Android en v1.1
```

Les commits d'une PR suivent le même format (première ligne), le détail va
dans le corps du message.

## Protection de `main` (réglage GitHub, à faire une fois)

Settings → Branches → *Add branch ruleset* (ou *Branch protection rule*) sur `main` :

- *Require a pull request before merging* (approbations : 0 si un seul mainteneur) ;
- *Require status checks to pass* : `linux`, `windows`, `secrets`, avec
  *Require branches to be up to date before merging* ;
- *Block force pushes* et interdiction de supprimer la branche ;
- *Do not allow bypassing the above settings* (s'applique aussi aux administrateurs).

Settings → Code security : activer *Secret scanning* et *Push protection*
(gratuits pour un dépôt public) en plus du job `secrets`.

## Publication d'une version

1. La PR monte `version` dans `Cargo.toml` et ajoute sa section en tête de
   `CHANGELOG.md` (vérifié par la CI).
2. Après le merge, le propriétaire lance `./release.sh` (Git Bash sous
   Windows) : il pose et envoie le tag `vX.Y.Z` sur `main`, après confirmation.
3. La CI compile l'installeur, le **signe** (secrets
   `TAURI_SIGNING_PRIVATE_KEY` et `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`, lus
   seulement sur un tag) et publie la release avec `latest.json`. Les
   applications installées la proposent alors à leurs utilisateurs.

Clé de signature des mises à jour (une fois, par le propriétaire uniquement) :

```bash
npx @tauri-apps/cli signer generate -w ~/.tauri/pdfeditor.key
```

Contenu de `~/.tauri/pdfeditor.key` → secret `TAURI_SIGNING_PRIVATE_KEY`,
son mot de passe → `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` (Settings → Secrets
and variables → Actions). La clé **publique** (`pdfeditor.key.pub`) va dans
`apps/desktop/tauri.conf.json` (`plugins.updater.pubkey`) ; elle n'est pas
secrète. Perdre la clé privée empêcherait toute mise à jour automatique des
versions déjà installées : la sauvegarder.

Clé de signature de l'APK Android (une fois, par le propriétaire uniquement ;
celle de VoiceToText peut servir) :

```bash
keytool -genkeypair -v -keystore pdfeditor.jks -alias pdfeditor \
  -keyalg RSA -keysize 4096 -validity 10000
base64 -w0 pdfeditor.jks    # Git Bash ; macOS : base64 -i pdfeditor.jks
```

Secrets GitHub Actions : `ANDROID_KEYSTORE_BASE64` (sortie de `base64`),
`ANDROID_KEYSTORE_PASSWORD`, `ANDROID_KEY_ALIAS` (`pdfeditor`),
`ANDROID_KEY_PASSWORD`. La CI les lit seulement sur un tag, signe l'APK et
publie `latest-android.json`. Android n'installe une mise à jour que signée
avec la même clé : la perdre oblige chaque utilisateur à désinstaller puis
réinstaller ; la sauvegarder, hors du dépôt.

## Secrets

- Clés API, jetons, mots de passe, clés privées et certificats ne vont
  **jamais** dans le dépôt, même dans un test ou un exemple.
- Les clés de l'utilisateur vivent dans le trousseau du système.
- Les tests génèrent leurs clés et certificats à l'exécution.
- La CI lit ses secrets dans les secrets GitHub Actions, et seulement dans
  les builds de tags (jamais dans les PR).
- Le `.gitignore` refuse `.env`, `*.key`, `*.pem`, `*.p12`, `*.pfx` ; le job
  `secrets` (gitleaks) analyse tout l'historique à chaque PR.
