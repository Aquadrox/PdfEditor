# PdfEditor

Éditeur de PDF open source pour Windows (puis Linux), écrit en Rust :

- **éditer** : ajouter du texte, des images, des annotations ; modifier le
  texte existant ; caviarder ; organiser les pages ;
- **signer** (signature numérique PAdES) et **verrouiller** le document ;
- lancer un **cycle de signature** entre plusieurs personnes, sans serveur ;
- faire **analyser**, **traduire** ou **modifier** tout ou partie du
  document par une **IA** au choix (DeepSeek, OpenAI, Mistral, OpenRouter,
  modèle local Ollama…), toujours avec aperçu et accord avant application.

Tout est réglable dans les Réglages.

> Projet en démarrage : voir l'avancement dans [docs/ROADMAP.md](docs/ROADMAP.md).

## Documentation

| Document | Contenu |
|---|---|
| [docs/FONCTIONS.md](docs/FONCTIONS.md) | Liste des fonctions et priorités |
| [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) | Couches, crates, choix techniques, tests, confidentialité |
| [docs/DESIGN.md](docs/DESIGN.md) | Direction visuelle, jetons, composants, rédaction |
| [docs/ROADMAP.md](docs/ROADMAP.md) | Étapes et avancement |
| [CONTRIBUTING.md](CONTRIBUTING.md) | Règles de contribution, protection de `main`, secrets |

## Installer

Les versions publiées sont dans les
[Releases](https://github.com/Aquadrox/PdfEditor/releases) : télécharger
l'installeur `PdfEditor_X.Y.Z_x64-setup.exe`. Ensuite, PdfEditor propose
lui-même ses mises à jour (vérifiées par signature, installées sur votre clic).

## Construire

Rust stable (voir `rust-toolchain.toml`). Le moteur PDF a besoin de la
bibliothèque PDFium, téléchargée (version figée, empreinte vérifiée) dans
`vendor/pdfium/` par :

```bash
./scripts/fetch-pdfium.sh
```

```bash
cargo test --workspace
```

Lancer l'application (sous Linux, paquets `libwebkit2gtk-4.1-dev` et
`librsvg2-dev` nécessaires) :

```bash
cargo run -p pdfeditor -- chemin/vers/document.pdf
```

Tests de l'interface (Node 22) :

```bash
cd apps/desktop/ui && node --test
```

```bash
cd apps/desktop/ui-tests && npm ci && npm test
```

## Licence

MIT. PDFium (moteur PDF) est distribué sous licence BSD-3 / Apache-2.0.
