# Architecture

## Vue d'ensemble

Application de bureau **Rust + Tauri 2** (Windows d'abord, Linux ensuite),
organisée en couches : un **domaine pur** au centre, des **adaptateurs**
autour (PDFium, cryptographie, LLM, système), une **couche application** qui
orchestre les cas d'usage, et une **interface web** mince.

```
┌──────────────────────────────────────────────────────────────────────┐
│ apps/desktop   Tauri 2 : fenêtres, commandes, protocole pde://       │
│                ui/ : HTML + CSS + JS (modules ES, sans framework)    │
└───────────────┬──────────────────────────────────────────────────────┘
                │ commandes Tauri (fines : désérialiser → appeler → sérialiser)
┌───────────────▼──────────────────────────────────────────────────────┐
│ pde-app        cas d'usage : session de document, sauvegarde auto,   │
│                orchestration édition / signature / cycle / IA        │
└──┬──────────┬───────────┬────────────┬─────────────┬─────────────────┘
   │          │           │            │             │
┌──▼─────┐ ┌──▼──────┐ ┌──▼────────┐ ┌─▼──────┐ ┌────▼────────┐
│pde-pdf │ │pde-sign │ │pde-workflow│ │pde-ai │ │pde-platform │   adaptateurs
│PDFium  │ │PAdES    │ │cycle      │ │LLM    │ │trousseau,   │
│        │ │lopdf    │ │           │ │       │ │chemins      │
└──┬─────┘ └──┬──────┘ └──┬────────┘ └─┬──────┘ └────┬────────┘
   └──────────┴───────────┴────────────┴─────────────┘
                │ implémentent les traits (ports) de…
┌───────────────▼──────────────────────────────────────────────────────┐
│ pde-core       domaine pur : géométrie, modèle de document,          │
│                opérations d'édition, historique, réglages, traits    │
│                — aucune dépendance OS, PDFium, réseau ou interface   │
└──────────────────────────────────────────────────────────────────────┘
```

Règle de dépendance : les flèches ne vont que vers le bas. `pde-core` ne
dépend de rien du projet ; un adaptateur ne dépend jamais d'un autre
adaptateur (il passe par un trait de `pde-core`) ; seul `apps/desktop` connaît Tauri.

## Crates

| Crate | Rôle | Dépendances principales | Arrive en |
|---|---|---|---|
| `pde-core` | Géométrie, unités, modèle du document (pages, blocs de texte, éléments), opérations d'édition, historique annuler / rétablir, réglages (TOML + validation), traits `PdfEngine`, `ChatModel`, `SecretStore`, `Clock` | `serde`, `toml`, `thiserror` | v0.0 |
| `pde-pdf` | Moteur PDF : ouverture, rendu, extraction du texte avec positions, application des opérations, enregistrement complet ou incrémental | `pdfium-render` (PDFium) | v0.1 |
| `pde-sign` | Signature PAdES, vérification, certificats (PKCS#12, auto-signé), DocMDP / FieldMDP, horodatage RFC 3161, écriture incrémentale | `lopdf`, RustCrypto (`cms`, `x509-cert`, `rsa`, `p256`, `sha2`, `p12-keystore`) | v0.4 |
| `pde-workflow` | Cycle de signature : plan, machine à états, manifeste embarqué, journal d'audit chaîné | `pde-core`, `serde_json`, `sha2` | v0.8 |
| `pde-ai` | Client LLM compatible OpenAI (repris de VoiceToText), profils, prompts, découpage du document, tâches analyse / traduction / modification, coûts | `ureq`, `serde_json` | v0.5 |
| `pde-platform` | Trousseau (Gestionnaire d'identifiants Windows, Secret Service Linux), chiffrement local (DPAPI / ChaCha20-Poly1305), dossiers de l'application | `windows`, `secret-service` | v0.1 |
| `pde-app` | Cas d'usage testables sans interface : `DocumentSession` (document + historique + sauvegarde auto), services signature, cycle, IA | tous les crates ci-dessus | v0.1 |
| `apps/desktop` | Tauri : commandes, événements, fenêtres, protocole `pde://` pour les images de pages, mises à jour | `tauri` | v0.1 |

## Choix techniques

| Sujet | Choix | Raison |
|---|---|---|
| Langage | Rust (édition 2024), toolchain stable | Règle du projet ; binaire unique, sûreté mémoire sur des fichiers non fiables |
| Interface | Tauri 2, HTML / CSS / JS en modules ES **sans framework ni étape de build** | Comme VoiceToText : léger (WebView2 déjà présent sur Windows), rien à installer pour développer. Si la complexité de l'éditeur l'exige, passage à un framework léger réévalué en v0.2 |
| Moteur PDF | **PDFium** via `pdfium-render` | Le moteur de Chrome : rendu fidèle, extraction du texte avec positions, création d'objets texte / image, polices embarquées, formulaires, enregistrement incrémental. Licence BSD-3 / Apache-2.0. Alternative pur Rust (`hayro`) trop jeune en 2026 pour un éditeur |
| Distribution de PDFium | Bibliothèque précompilée (`bblanchon/pdfium-binaries`), version figée et SHA-256 vérifié, téléchargée par la CI, livrée avec l'installeur (~6 Mo) | Pas de compilation de PDFium (des heures) ; pas de binaire dans le dépôt |
| Fil d'exécution PDFium | Un **thread dédié** qui possède PDFium, piloté par messages (acteur) ; l'interface n'attend jamais | PDFium n'est pas thread-safe ; rendu et enregistrement ne bloquent pas l'interface |
| Images des pages | Protocole Tauri `pde://doc/{id}/page/{n}?scale=…` ; rendu en tuiles au-delà d'un seuil de zoom ; cache LRU borné en mémoire | Pas de base64 dans l'IPC ; le WebView met en cache ; mémoire maîtrisée sur les gros documents |
| Couches d'affichage | Image de la page + couche texte invisible (sélection, recherche, copie) + couche d'édition (éléments ajoutés, poignées) en HTML/SVG | Édition fluide sans re-rendu PDFium à chaque déplacement de souris ; même principe que pdf.js |
| Modèle d'édition | Le PDF d'origine reste intact en mémoire ; les modifications sont une **liste d'opérations** (patron Commande) appliquée à l'enregistrement | Annuler / rétablir trivial, sauvegarde automatique = journal d'opérations, aucune perte du fichier d'origine |
| Écriture des signatures | `lopdf` + écrivain de **mise à jour incrémentale** maison (nouveaux objets, xref, trailer ajoutés à la fin) | La signature exige un contrôle exact des octets (`/ByteRange`, `/Contents` réservé) que PDFium n'offre pas |
| Cryptographie | RustCrypto (pur Rust) : CMS / PKCS#7, X.509, RSA, ECDSA P-256, SHA-256 | Portable, testable sous Linux, pas d'OpenSSL à livrer |
| IA | API compatible OpenAI (`/chat/completions`), client bloquant `ureq` sur un thread de travail, réponses en flux (SSE) | Code éprouvé de VoiceToText ; un seul format couvre DeepSeek, OpenAI, Mistral, GLM, OpenRouter, Ollama… |
| Réglages | TOML dans le dossier de configuration (`%APPDATA%\PdfEditor\config.toml`), un module par rubrique dans `pde-core::settings` | Lisible et éditable à la main ; validation testée |
| Secrets | Trousseau du système (code de VoiceToText `secrets_windows.rs` / `secrets_linux.rs`) | Jamais en clair sur le disque, jamais dans le dépôt |
| Journaux | Fichier tournant dans le dossier local de l'application ; **jamais le contenu des documents** ni les prompts (longueurs et durées seulement) | Diagnostic sans fuite de données |
| Mises à jour | Module de mise à jour Tauri, paquets signés minisign (clé privée du propriétaire dans les secrets GitHub), comme VoiceToText | Processus déjà validé |

## Modèle d'édition

```
DocumentSession
├── source : octets du PDF d'origine (jamais modifiés)
├── engine : poignée vers le document ouvert dans PDFium (thread dédié)
├── history : Vec<Operation> + curseur (annuler / rétablir)
└── assets : images et polices ajoutées, référencées par empreinte SHA-256
```

`Operation` est une énumération sérialisable (sauvegarde automatique en JSON) :
`AddText`, `AddImage`, `AddAnnotation`, `MoveElement`, `EditText`,
`DeleteObject`, `Redact`, `InsertPage`, `DeletePage`, `RotatePage`,
`MovePage`, `MergeDocument`, `FillField`, `SetMetadata`…
Chaque opération sait se **valider** contre le document (page existante,
rectangle dans la page…) et produire son **inverse** ; les tests vérifient
`appliquer ∘ inverse = identité` sur des documents de référence.

À l'enregistrement, `pde-pdf` rejoue les opérations sur une copie dans
PDFium, écrit un fichier temporaire, le relit pour vérifier qu'il s'ouvre, puis
remplace le fichier cible (renommage atomique). Si le document contient des
signatures, l'enregistrement est **incrémental** (F-03) ; s'il est verrouillé
(DocMDP sans modification permise), l'édition est désactivée.

## Signature et verrouillage

1. Le document (avec les modifications en cours) est enregistré.
2. `pde-sign` ajoute une mise à jour incrémentale : champ de signature,
   apparence (XObject), dictionnaire `/Sig` avec `/Contents` réservé et
   `/ByteRange` ; pour le verrouillage, référence `/DocMDP` (`/P 1` aucune
   modification, `/P 2` formulaires et signatures).
3. Empreinte SHA-256 des octets couverts par `/ByteRange` → CMS SignedData
   détaché (attributs signés PAdES : `signingCertificateV2`, `messageDigest`),
   jeton d'horodatage RFC 3161 en attribut non signé si configuré.
4. Le CMS est écrit dans `/Contents` ; le fichier est relu et la signature
   **vérifiée** avant d'être présenté comme signé.

Clés privées : certificat auto-signé généré → clé chiffrée par le système
(DPAPI / trousseau) ; PKCS#12 → le fichier reste où il est, le mot de passe
est demandé ou mémorisé dans le trousseau sur demande. Jamais de clé dans
les journaux, jamais de clé de test réelle dans le dépôt (clés de test
générées à l'exécution des tests).

## Cycle de signature

Sans serveur : le **plan du cycle** (signataires, ordre, champs, état,
journal d'audit) est un fichier JSON joint au PDF (`/EmbeddedFiles`), ajouté
par mise à jour incrémentale. L'initiateur pose une signature de
certification `/P 2` (signatures permises) ; chaque signataire signe son
champ (signature d'approbation) et met à jour le plan ; le dernier verrouille
(`/DocMDP` ou `/Lock` sur les champs). Le transport (courriel préparé,
dossier partagé, plus tard serveur) est un trait `CycleTransport` : le cœur du
cycle ne sait pas comment le fichier voyage. La machine à états
(`Brouillon → En cours → Terminé | Refusé | Expiré`) est dans `pde-workflow`
et testée exhaustivement.

## IA

```
Portée (sélection | pages | tout)
  → extraction en blocs (id, page, rectangle, texte, style)       pde-pdf
  → découpage en lots sous la limite de jetons, numéros de page    pde-ai
  → prompt système (éditable) + données balisées, désamorcées      pde-ai
  → ChatModel (trait) → client OpenAI-compatible | faux modèle (tests)
  → réponse validée (JSON strict pour traduction / modification)
  → aperçu (texte, différences) → accord de l'utilisateur
  → opérations EditText / Redact ajoutées à l'historique (annulables)
```

- **Analyse / questions** : réduction par lots (résumé de chaque lot, puis
  synthèse) ; chaque lot porte ses numéros de page, les citations de la
  réponse sont vérifiées (page existante) avant d'être affichées comme liens.
- **Traduction** : par blocs, réponse JSON `{id, texte}` ; chaque bloc traduit
  remplace l'original dans son rectangle, police réduite jusqu'à un minimum
  réglable, débordement signalé à l'utilisateur.
- **Modification** : même format ; seules les propositions sur des blocs
  existants de la portée sont acceptées ; affichage en différences.
- **Sécurité** : le document est une donnée (`<document>…</document>`),
  balises désamorcées dans le texte (repris de VoiceToText), réponse hors
  format rejetée ; aucun envoi sans fournisseur configuré et, au-delà du
  seuil, sans confirmation ; documents signés : modification IA refusée tant
  qu'elle invaliderait une signature.

## Reprise de VoiceToText

Code **copié puis adapté** (pas de dépendance entre les dépôts : chacun évolue
à son rythme).

| Source VoiceToText | Destination | Adaptation |
|---|---|---|
| `crates/vtt-llm/src/lib.rs` (client, erreurs HTTP, pistes de correction, `/models`, désamorçage des balises, lecture de l'`usage`) | `pde-ai` (découpé : `client`, `errors`, `hints`, `models`, `sanitize`) | Prompts de dictée retirés ; ajout du flux SSE et du JSON strict |
| `vtt-core::config` (`LlmConfig`, `LlmProfile`, `Pricing`, `PromptPreset`, contrôle de l'adresse, envoi de la clé seulement en HTTPS ou vers ce PC) | `pde-core::settings::ai` | Prompts par action IA |
| `vtt-core::usage` (jetons, coût par jour) | `pde-core::usage` | Plafond mensuel |
| `vtt-platform` `secrets_windows.rs`, `secrets_linux.rs` | `pde-platform::secrets` | Préfixe `PdfEditor/` |
| `apps/desktop/ui/settings.js` (préréglages fournisseurs, test des réglages), `prompts.js` | `apps/desktop/ui/settings/ai/` | Découpé en modules |
| `logging.rs`, `crash.rs`, `paths.rs`, `updates.rs` | `apps/desktop/src/` | Noms de dossiers |
| `release.sh`, `.github/scripts/changelog.py`, jobs de release de la CI | mêmes emplacements | Plateformes Windows / Linux |
| Jetons de couleur de `app.css` | `apps/desktop/ui/styles/tokens.css` | Voir `DESIGN.md` |

## Qualité et tests

- **Découpage** : un module = une responsabilité ; repère de **400 lignes de
  code** par fichier (tests à part) au-delà duquel on découpe ; fonctions courtes ;
  commandes Tauri sans logique (elles appellent `pde-app`).
- **Tests unitaires** dans chaque module (`#[cfg(test)]`), **tests
  d'intégration** dans `crates/*/tests/` sur des PDF de référence
  (`tests/fixtures/`, petits, générés par le projet ou sous licence libre).
- **Non-régression** : tout bogue corrigé arrive avec le test qui le
  reproduit ; tests de rendu comparés à des images de référence (tolérance
  par pixel) ; propriétés (`proptest`) sur la géométrie et l'historique.
- **Signatures** : chaque test de signature vérifie le fichier avec notre
  vérificateur **et** un outil indépendant dans la CI (`pdfsig` de Poppler,
  `qpdf --check`).
- **IA** : aucune vraie API en CI (pas de secret) ; un `FakeChatModel` rejoue
  des réponses scriptées (y compris malformées ou malveillantes) pour tester
  découpage, prompts, validation et désamorçage.
- **Interface** : modules JS purs testés avec `node --test` (sans
  dépendance) ; parcours principaux avec Playwright à partir de la v0.3.
- **CI** (obligatoire pour merger) : `linux` (format, clippy `-D warnings`,
  tests), `windows` (clippy, tests, puis installeur), `secrets` (gitleaks sur
  tout l'historique). À venir : `cargo-deny` (licences, vulnérabilités), couverture.

## Confidentialité

- Les documents ne quittent jamais le PC, **sauf** le texte envoyé au
  fournisseur IA choisi pour une action IA demandée par l'utilisateur, limité
  à la portée choisie. Avec un modèle local (Ollama, LM Studio), rien ne sort.
- Avant le premier envoi à un fournisseur, l'application affiche ce qui part et
  où ; option « confirmer avant chaque envoi ».
- La clé API n'est envoyée qu'en HTTPS, ou en HTTP vers ce PC.
- Journaux et suivi de consommation : aucun texte de document.
- Le contenu des PDF est affiché dans le WebView sous forme d'images et de
  `textContent` uniquement : un PDF piégé ne peut pas injecter de HTML ni de
  script ; le JavaScript embarqué dans les PDF n'est jamais exécuté.
- Fichiers non fiables : ouverture et rendu isolés sur le thread PDFium, une
  erreur de lecture n'arrête pas l'application ; analyse par fuzzing prévue.

## Points d'attention

| Risque | Parade |
|---|---|
| Modifier le texte existant : polices réduites à un sous-ensemble sans les caractères nécessaires | Police de remplacement embarquée (Noto, licence OFL) appliquée au bloc, signalée à l'utilisateur |
| Traduction plus longue que l'original | Ajustement de la taille jusqu'à un minimum réglable, puis signalement du débordement |
| Modifier un document signé invalide les signatures | Enregistrement incrémental ; avertissement avant toute modification non permise par DocMDP ; lecture seule si verrouillé |
| Gros documents (centaines de pages, scans) | Rendu à la demande, tuiles, cache borné, miniatures en basse résolution |
| PDF malformés ou malveillants | PDFium (éprouvé par Chrome), aucune exécution de JavaScript PDF, tests de fuzzing de nos propres lecteurs |
