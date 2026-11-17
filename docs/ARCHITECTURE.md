# Architecture

## Vue d'ensemble

Application **Rust + Tauri 2** (Windows d'abord, puis Linux, puis Android en v1.1),
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
| `pde-fonts` | Polices Noto embarquées (Sans, Serif, Sans Mono : latin étendu, grec, cyrillique ; OFL), caractères couverts, métriques | `skrifa` | v0.2 |
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
| Distribution de PDFium | Bibliothèque précompilée (`bblanchon/pdfium-binaries`, build 8125), version figée et SHA-256 vérifié par `scripts/fetch-pdfium.sh`, livrée avec l'installeur (~6 Mo) ; cherchée dans `PDE_PDFIUM_DIR`, puis à côté de l'exécutable, puis (développement) dans `vendor/pdfium/` | Pas de compilation de PDFium (des heures) ; pas de binaire dans le dépôt |
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
| Android (v1.1) | Tauri 2 en version mobile : mêmes crates Rust, même interface web (WebView Android) adaptée au tactile ; PDFium précompilé pour Android (arm64) ; secrets dans le Keystore Android (`pde-platform`) ; mises à jour par APK signé et `latest-android.json` (taille, SHA-256), le module de mise à jour de Tauri n'existant pas sur mobile | Un seul code pour PC et téléphone. VoiceToText a dû passer par Kotlin pour son clavier Android ; un éditeur de PDF n'en a pas besoin. Garde-fou dès la v0.2 : la CI compile le cœur pour Android |

## Modèle d'édition

```
DocumentSession
├── source : octets du PDF d'origine (jamais modifiés)
├── engine : poignée vers le document ouvert dans PDFium (thread dédié)
├── history : Vec<Operation> + curseur (annuler / rétablir)
└── assets : images (et plus tard polices) ajoutées, par identifiant
```

`Operation` est une énumération sérialisable (sauvegarde automatique en JSON) :
`AddText`, `AddImage`, `AddAnnotation`, `MoveElement`, `EditText`,
`DeleteObject`, `Redact`, `InsertPage`, `DeletePage`, `RotatePage`,
`MovePage`, `MergeDocument`, `FillField`, `SetMetadata`…
Chaque opération sait se **valider** contre le document (page existante,
rectangle dans la page…) et produire son **inverse** ; les tests vérifient
`appliquer ∘ inverse = identité` sur des documents de référence.

À chaque opération, `pde-pdf` recharge le PDF d'origine et y pose tous les
éléments (`pde-core::edit`) : l'image affichée est exactement ce qui sera
enregistré, et annuler revient à reposer la liste précédente.

Pages (P-01, P-02, `pde-core::edit::pages`) : un **plan des pages** décrit
le document arrangé à partir de l'original (page d'origine ou page blanche,
quarts de tour ajoutés). Insérer, supprimer, dupliquer, déplacer et pivoter
sont des opérations de l'historique comme les autres ; les éléments ajoutés
suivent leur page (déplacés, copiés, retirés, tournés avec elle — sauf le
texte ajouté, souligné ou barré, qui refuse encore la rotation). `pde-pdf`
(`arrange.rs`) range les pages avant d'y poser les éléments : la plus longue
suite de pages d'origine déjà dans le bon ordre reste formée des mêmes objets
PDF (signets, liens internes et champs de formulaire les suivent) ; les
autres sont des copies prises dans un second exemplaire de l'original.
PDFium-render n'offre pas de déplacement sans recopie hors `unsafe`. Quand
le plan change, `pde-app` décrit à nouveau le document (tailles, signets
renvoyés à leur nouvelle place) et l'interface refait la visionneuse, les
miniatures et la grille du mode Pages.

Pages d'autres PDF (P-03, `pde-core::edit::import`) : un PDF inséré est lu
une fois, décrit par le moteur (taille des pages) puis gardé en mémoire
pour la session sous un `ImportId`, comme une image ; le plan désigne ses
pages (`PageSource::Imported`), que `pde-pdf` copie à chaque rangement. Le
moteur reçoit tout ce qu'il applique dans une `Edition` (plan, éléments,
images, PDF insérés) ; `PdfEngine::export` produit un autre document à
partir de l'original (extraire des pages) sans toucher au document affiché.

Images ajoutées (E-02) : `pde-app::images` reconnaît le format au contenu
(PNG, JPEG, WebP, crate `image` en Rust pur), borne la taille avant de
décoder (64 Mo, 60 mégapixels) et applique l'orientation EXIF. Un JPEG est
inclus tel quel dans le PDF (aucune recompression) ; les autres images sont
écrites en pixels avec leur transparence (masque). Le contenu est importé
une fois par document (`ImageId`) et partagé par toutes les copies du cadre
(`ImageBox` : position, taille, quarts de tour, opacité).

Annotations dessinées (E-04, `pde-core::edit::annotation`) : formes,
traits à main levée et marquages du texte sont des éléments comme les
autres, tracés par `pde-pdf` dans le contenu de la page (chemins PDFium ;
surlignage en mode de fusion « produit » pour garder le texte noir). Le
calcul des tracés (pointe de flèche, ellipse en courbes de Bézier, bandes
de soulignement) est dans `pde-core`, testé sans moteur. Dans l'interface,
le marquage suit la couche texte : début et fin pris au caractère sous la
souris, bandes calées sur les boîtes du PDF. Les notes (`NoteBox`) sont,
elles, de vraies annotations PDF « Texte » (commentaire, auteur `T`,
couleur), recréées à chaque pose comme le reste.

Sauvegarde automatique (F-02, `pde-app::recovery`) : chaque lancement écrit
dans `recovery/<session>/doc-<n>/` (dossier de données) un `manifest.json`
(chemin, empreinte SHA-256 du PDF d'origine, plan des pages s'il a changé,
PDF insérés désignés par chemin et empreinte, éléments) et ses images ; le
PDF n'est jamais copié, un document protégé par mot de passe n'est jamais
sauvegardé. La session est verrouillée (`session.lock`, `File::try_lock`)
tant que le lancement tourne : au démarrage, une session qu'on peut
verrouiller vient d'un lancement interrompu et ses documents sont proposés.
La reprise n'a lieu que sur un fichier de même empreinte ; chaque élément
redevient une opération annulable. Enregistrer, fermer le document ou
quitter en confirmant efface la sauvegarde ; une sortie sans confirmation
(redémarrage de mise à jour) écrit une dernière sauvegarde. À
l'enregistrement, le contenu produit est relu par PDFium (même mot de passe,
même nombre de pages), écrit dans un fichier temporaire puis renommé à la
place du fichier cible. Un document protégé par mot de passe le reste. Si le document contient des
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
  dépendance) ; parcours dans Chromium avec Playwright et un faux Tauri
  (`apps/desktop/ui-tests`).
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
