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
| `pde-core` | Géométrie, unités, modèle du document (pages, blocs de texte, éléments), opérations d'édition, historique annuler / rétablir, réglages (TOML + validation), traits `PdfEngine`, `DataProtector`, plus tard `ChatModel`, `Clock` | `serde`, `toml`, `thiserror` | v0.0 |
| `pde-pdf` | Moteur PDF : ouverture, rendu, extraction du texte avec positions, application des opérations, formulaires et propriétés, enregistrement complet ou incrémental | `pdfium-render` (PDFium), `lopdf` (structure du formulaire, valeurs, dictionnaire d'information) | v0.1 |
| `pde-fonts` | Polices Noto embarquées (Sans, Serif, Sans Mono : latin étendu, grec, cyrillique ; OFL), caractères couverts, métriques | `skrifa` | v0.2 |
| `pde-sign` | Signature PAdES, vérification, identités (PKCS#12, auto-signé), DocMDP (plus tard FieldMDP, horodatage RFC 3161), écriture incrémentale ; ni PDFium ni système | `lopdf`, RustCrypto (`cms`, `x509-cert`, `x509-tsp`, `rsa`, `p256`, `p384`, `sha2`), `p12-keystore` | v0.4 |
| `pde-workflow` | Cycle de signature : plan, machine à états, manifeste embarqué, journal d'audit chaîné | `pde-core`, `serde_json`, `sha2` | v0.8 |
| `pde-ai` | Client LLM compatible OpenAI (repris de VoiceToText), réponses en flux, transport HTTP abstrait (`ureq` en option `http`, pour l'application seulement), prompts, découpage du document, tâches analyse / questions (plus tard traduction, modification), estimation des coûts | `serde_json`, `ureq` (option) | v0.5 |
| `pde-platform` | Dossiers de l'application, protection des données par le système (`DataProtector`) : DPAPI de Windows ; sous Linux, chiffrement authentifié (ChaCha20-Poly1305) avec une clé gardée dans le trousseau Secret Service ; plus tard le trousseau pour les clés API | `windows`, `secret-service`, `chacha20poly1305` | v0.1 |
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

Caviardage (E-07, `pde-core::edit::redact`, `pde-pdf` `writer/redact.rs`) :
une zone caviardée est un élément comme les autres, posé en dernier. Sur sa
page, `pde-pdf` rend la page, retire du contenu chaque objet qui touche une
zone, le remplace par la partie correspondante du rendu (zones noircies),
supprime les annotations touchées et pose un aplat noir : le contenu
caviardé n'existe plus dans le document, le reste garde son aspect.
L'interface demande confirmation avant d'écrire des zones nouvelles.

Images d'origine (E-06, `pde-core::edit::existing`, `pde-pdf`
`writer/existing.rs`) : une image du document est désignée par son rang
parmi les objets de la page arrangée, avant tout ajout (stable, puisque le
moteur repart de l'original), et par son cadre d'origine, vérifié à chaque
pose. L'élément `ExistingImage` porte le nouveau cadre (déplacement et
mise à l'échelle appliqués à sa matrice, orientation gardée) ou rien
(supprimée) ; il suit sa page comme les autres. Remplacer = la supprimer et
poser une image ajoutée dans son cadre, en une opération
(`Edits::add_and_update`). L'interface ne crée l'élément qu'à la première
modification : avant, l'image est proposée à la sélection telle que la
décrit `PdfEngine::page_images`.

Texte du document (E-05, `pde-core::edit::original_text`, `pde-pdf`
`writer/blocks.rs` et `writer/original_text.rs`) : `PdfEngine::page_text_blocks`
regroupe les objets texte droits d'une page en lignes (même police, corps
et couleur, côte à côte) ; une ligne est désignée par le rang de ses objets
et vérifiée par sa place d'origine, comme les images d'origine. L'élément
`OriginalText` porte le nouveau texte (vide : supprimée) et le style de
remplacement. À la pose, le premier objet reçoit le texte si sa police sait
l'écrire (police non embarquée, ou caractères déjà écrits avec elle sur la
page ; texte relu pour confirmer), les autres sont retirés ; sinon tous sont
retirés et le texte est écrit comme une zone ajoutée, avec la police
standard la plus proche (`Placed::replaced_font`, signalé par l'interface).
Les retraits d'images et de texte d'une page se font ensemble, du dernier
rang au premier (`writer/existing.rs`).

Formulaires et propriétés (E-08, E-11, `pde-core::edit::form`, `pde-pdf`
`form/`) : la structure du formulaire (champs, genres, options, valeur) est
lue avec `lopdf`, la place des widgets par PDFium sur les pages arrangées.
Les valeurs saisies et les propriétés sont écrites avec `lopdf` sur le
contenu enregistré par PDFium (valeur, état des cases, apparence retirée,
`NeedAppearances`), puis le document est rechargé : PDFium redessine les
champs. L'aplatissement passe par PDFium. Un document chiffré est refusé
pour ces deux opérations.

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

On signe le **fichier enregistré**, jamais l'état en mémoire : le document
ne doit pas avoir de modifications en attente (l'interface propose de les
enregistrer d'abord), et le fichier sur le disque doit avoir l'empreinte de
la dernière ouverture, du dernier enregistrement ou de la dernière signature.

1. `pde-sign` lit la structure avec `lopdf` (catalogue, formulaire, pages,
   champs existants) puis écrit une **mise à jour incrémentale** à la fin du
   fichier (`pdf/incremental.rs`) : les objets modifiés (page, `/AcroForm`,
   catalogue) et nouveaux (widget, dictionnaire `/Sig`, apparence), puis une
   section de références croisées du même style que le fichier (table
   `xref`, ou flux de références croisées) et un `trailer` avec `/Prev`.
   Aucun octet existant ne bouge : les signatures précédentes restent valides.
2. Le dictionnaire `/Sig` (`/SubFilter /ETSI.CAdES.detached`) est écrit à
   la main avec deux trous de taille fixe : `/ByteRange` et `/Contents`
   (place pour le CMS : certificats + 3 Ko). Pour verrouiller (DocMDP), il
   porte un `/Reference` (`/P 1` aucune modification, `/P 2` formulaires et
   signatures) et le catalogue reçoit `/Perms /DocMDP` ; seule la première
   signature d'un document peut verrouiller.
3. Empreinte (SHA-256, SHA-384 pour P-384) de tout le fichier sauf `<…>` de
   `/Contents` → CMS `SignedData` détaché (`cms/build.rs`) : attributs
   signés `contentType`, `messageDigest`, `signingCertificateV2` ; heure de
   signature dans `/M` (PAdES B-B). Pas encore d'horodatage (S-08).
4. Le CMS est écrit en hexadécimal dans `/Contents`. `pde-app` relit le
   fichier : notre vérificateur doit trouver la nouvelle signature valide et
   couvrant tout le fichier, et PDFium doit l'ouvrir ; alors seulement le
   fichier est remplacé (écriture atomique) et l'interface le rouvre.

Cadre d'une signature visible : l'interface envoie un cadre en points de la
page affichée (origine en haut à gauche), `pde-app` le ramène en bas à
gauche, `pde-sign` le place dans l'espace du PDF selon la `/CropBox` et
`/Rotate` de la page (`pdf/geometry.rs`). L'apparence est un XObject de
texte (Helvetica, WinAnsi) rédigé par l'interface (nom, organisation, date
locale) ; l'apparence manuscrite viendra avec S-03.

Vérification (`verify.rs`) : pour chaque champ `/FT /Sig` du formulaire,
`/ByteRange` cohérent (commence à 0, le trou ne contient que de
l'hexadécimal), empreinte, signature des attributs (RSA PKCS#1 v1.5, ECDSA
P-256 / P-384), certificat désigné par `signingCertificateV2` ; « document
complété depuis » si des octets suivent la révision signée. La confiance
dans l'émetteur (magasin de certificats du système) n'est pas encore
vérifiée : un certificat auto-signé est présenté comme « personnel ».
Les signatures sont lues à l'ouverture et après chaque enregistrement.

Identités (`pde-app::signing::identities`) : un fichier JSON par identité
dans `identities/` du dossier de données (certificats, clé PKCS#8 chiffrée
par `DataProtector`) ; jamais de clé en clair sur le disque ni dans les
journaux (`PrivateKey` n'affiche que son algorithme). Le mot de passe d'un
PKCS#12 ne sert qu'à l'import. Les tests génèrent leurs clés et certificats
à l'exécution (aucun dans le dépôt).

Apparence manuscrite (S-03) : l'interface dessine la signature (tracé,
nom dans une police manuscrite, image importée dont le papier clair devient
transparent) sur un canevas, la détoure et l'envoie en PNG ; `pde-app`
la décode (taille bornée), `pde-sign` la pose en image PDF (`DeviceRGB` et
`/SMask`, compressée) au-dessus des lignes de texte. Les modèles (« Mes
signatures ») sont des PNG dans `signatures/` du dossier de données.

Horodatage (S-08) : après la signature, l'empreinte SHA-256 de la valeur
de signature part vers le serveur (`TimeStampReq` avec nonce, `x509-tsp`) ;
le jeton reçu est vérifié (empreinte, nonce, signature CMS du serveur) puis
ajouté en attribut non signé `timeStampToken`. Le transport est un trait
de `pde-sign` (`TimestampAuthority`) ; `pde-app` fournit le client HTTP
(`ureq` sans TLS : le jeton est signé, et rien de C à compiler pour
Android). La signature se fait hors du verrou des documents : l'appel réseau
ne fige pas l'affichage.

Enregistrer un document signé (F-03, `pde-sign::append_revision`) : PDFium
réécrit tout le document en gardant les numéros d'objets ; on compare
chaque objet au fichier signé **sur le fond** (ordre des clés, écriture des
nombres et des chaînes, compression des flux ignorés) et seuls les objets
changés ou nouveaux sont ajoutés en mise à jour incrémentale à la suite du
fichier signé (racine et `/Info` du document réécrit). Les dictionnaires de
signature ne sont jamais réécrits. `pde-app` refuse d'écrire si une
signature valide cessait de l'être, ou si le fichier a changé sur le disque.

Modifications après une signature (`pde-sign::changes`) : la révision
signée (les octets jusqu'à la fin de `/ByteRange`) est relue et comparée
au document, sur le fond. Chaque objet changé est classé : champ dont seuls
`/V`, `/AS`, `/AP` changent (remplissage), signature, annotation, page dont
le contenu ou les attributs changent, arbre des pages, `/Info`, autre ; le
formulaire et le catalogue ne sont que des conteneurs. Le résultat est
confronté à la certification (DocMDP) : `/P 2` permet remplissage et
signatures, `/P 1` rien.

Documents protégés : `lopdf` déchiffre avec le mot de passe ; chaque objet
ajouté est chiffré avec la clé du document (chaînes en hexadécimal), sauf
le `/Contents` des signatures (ISO 32000-1, 7.6.2), et le `trailer` reprend
`/Encrypt` et le premier `/ID` (dont dépend la clé). Le CMS est lu
directement dans les octets du fichier, jamais chiffrés.

Limite connue : la confiance dans l'émetteur d'un certificat n'est pas
vérifiée (S-09 / S-10).

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

- **Transport** : `pde-ai` ne parle qu'au trait `Transport` ; le transport
  `ureq` (HTTPS par rustls) est une option (`http`) activée par
  l'application de bureau. Le cœur (`pde-app`, `pde-ai` sans option) reste
  sans code C, donc compilable pour Android ; les tests rejouent des
  réponses HTTP écrites à la main (`FakeTransport`).
- **Service** (`pde-app::ai`) : clé par fournisseur au trousseau
  (`SecretStore` de `pde-core`, `ai-key/<hôte>`), estimation avant l'envoi,
  accord gardé par fournisseur et seuil de coût, plafond mensuel,
  consommation dans `ai-usage.json` (compteurs et noms d'hôtes seulement),
  profil de secours si le fournisseur échoue. L'action tourne sur un fil à
  part ; la réponse arrive à l'interface par des événements (`ai-event`,
  `ai-done`) et s'annule (`CancelFlag`).
- **Analyse / questions** : réduction par lots (résumé de chaque lot, puis
  synthèse) ; chaque lot porte ses numéros de page, les citations de la
  réponse sont vérifiées (page existante) avant d'être affichées comme liens.
- **Traduction** : par blocs, réponse JSON `{id, texte}` ; chaque bloc traduit
  remplace l'original dans son rectangle, police réduite jusqu'à un minimum
  réglable, débordement signalé à l'utilisateur. Les blocs sont les lignes
  de texte d'origine de E-05 (`viewer::translation::source_blocks`) ; le
  lot, la consigne (langue, glossaire) et la lecture stricte de la réponse
  sont dans `pde-ai::translate` ; le corps est ajusté d'après la largeur
  mesurée avec la police Noto du style (`pde-fonts`), une marge de 2 %
  couvrant l'écart avec la police d'origine. La pose est une opération
  groupée (`Edits::grouped`) ; en bilingue, les pages sont d'abord dupliquées
  et la traduction va sur les copies.
- **Modification** : même format ; seules les propositions sur des blocs
  existants de la portée sont acceptées ; affichage en différences.
- **Sécurité** : le document est une donnée (`<document>…</document>`),
  balises désamorcées dans le texte (repris de VoiceToText), réponse hors
  format rejetée ; aucun envoi sans fournisseur configuré et, au-delà du
  seuil, sans confirmation ; documents signés : modification IA refusée tant
  qu'elle invaliderait une signature.

### Assistant d'actions (IA-15 à IA-17)

Demander à l'IA « caviarde-moi ça » sur une zone choisie : elle propose
l'action, ou explique comment la faire. Méthode retenue : un **catalogue
d'actions** unique et l'**appel d'outils** (function calling) de l'API
compatible OpenAI ; les « compétences » sont des **recettes** ; MCP n'est
qu'un adaptateur de plus sur le même catalogue.

```
Sélection (page, zone, texte) + demande de l'utilisateur
  → recette éventuelle : consigne + actions permises            pde-ai
  → contexte : demande, texte de la sélection (donnée balisée),
    outils = actions permises du catalogue (JSON Schema)        pde-app::actions
  → ChatModel avec `tools` → appels d'outils proposés
  → contrôle : schéma, portée (pages et zones de la sélection),
    action permise par la recette ; sinon rejeté
  → aperçu sur la page (zones en pointillés) + texte d'explication
  → Appliquer : une opération groupée de l'historique (Batch), annulable ;
    les confirmations propres à l'action restent (caviardage)
```

- **Catalogue** (`pde-app::actions`) : chaque action (caviarder une zone ou
  un texte, masquer, surligner, ajouter une note, pivoter, supprimer ou
  extraire des pages, chercher…) a un nom, une explication en français, ses
  paramètres en JSON Schema, son contrôle et son exécution par les cas
  d'usage existants. La palette de commandes, l'aide « Comment faire ? »,
  les outils de l'IA et le serveur MCP en sont tirés : rien à maintenir en
  double, et un test vérifie que chaque action a son explication, son schéma
  et son outil dans la barre.
- **Assistant** (v0.0.23) : la tâche `Assist` de `pde-ai` envoie une
  requête (début du document, demande) avec les outils tirés du catalogue
  (`actions::assistant::tools`) ; les appels reçus sont lus en
  `Proposal` (`actions::assistant::parse`), puis la visionneuse
  (`viewer::assistant`) cherche le texte visé dans les pages de la portée
  et rend l'aperçu. Le modèle désigne du texte, jamais des coordonnées :
  une zone vient toujours de la recherche dans le document. « Appliquer »
  renvoie les propositions cochées, vérifiées à nouveau, appliquées par
  `Edits::grouped` (ajouts, puis rotations, puis suppressions) : une seule
  entrée de l'historique, rien d'appliqué si une étape échoue.
- **Palette** (v0.0.22) : l'interface reçoit le catalogue (`action_catalog`)
  et le cherche elle-même (`features/palette`). Elle exécute une action par
  son raccourci ou en actionnant son outil, après être passée dans son mode ;
  « Comment faire ? » met l'outil en évidence sans rien exécuter. Le
  catalogue est copié dans `ui-tests/fixtures/action-catalog.json` pour les
  tests de l'interface, et un test de `pde-app` vérifie qu'ils sont
  identiques.
- **Pourquoi l'appel d'outils** : il est géré par la plupart des fournisseurs
  compatibles (OpenAI, DeepSeek, Mistral, GLM, OpenRouter, modèles locaux
  récents via Ollama) ; la réponse est structurée, donc vérifiable avant tout
  aperçu. Un modèle sans outils répond en texte : l'assistant explique alors
  et renvoie à l'outil, sans rien proposer d'appliquer.
- **Recettes** (IA-16) : fichiers TOML (consigne, actions permises, portée
  par défaut), intégrées en lecture seule et personnelles dans le dossier de
  données ; c'est la forme « compétence sur mesure », sans code exécutable.
  Format et vérification dans `pde-core::recipe`, rangement dans
  `pde-app::recipes` (les intégrées sont des fichiers TOML du dépôt, lus au
  même format). Avec une recette, `app::ai` place sa consigne avant la
  demande et n'offre au modèle que ses actions ; une proposition hors de
  ces actions est écartée. Une recette importée reste une consigne
  d'utilisateur : elle ne peut rien appliquer sans aperçu ni accord.
- **Serveur MCP** (IA-17, plus tard) : `pdfeditor --mcp` (stdio) ou serveur
  local, désactivé par défaut ; il expose le catalogue tel quel (outils MCP =
  actions) sur le document ouvert. Le retenir comme cœur aurait imposé un
  protocole de plus à l'assistant intégré sans rien lui apporter ; comme
  adaptateur, il coûte peu une fois le catalogue en place.
- **Sécurité** : seule la demande de l'utilisateur est une consigne ; le
  texte du document reste une donnée et ne peut pas déclencher d'action, car
  rien n'est appliqué sans aperçu ni accord. « Masquer » (aplat, contenu
  conservé) et « caviarder » (suppression réelle) sont deux actions
  distinctes, et l'assistant signale la différence quand la demande parle
  de confidentialité.
- **Voix** : le champ de demande accepte la dictée du système (Windows+H) et
  celle de VoiceToText ; une transcription intégrée reprendrait son moteur
  (copie adaptée, comme le reste) si l'usage le justifie.
- **Tests** : `FakeChatModel` qui rejoue des appels d'outils ; appels
  invalides, hors portée ou non permis par la recette rejetés ; parcours
  Playwright de l'aperçu et de l'acceptation.

## Reprise de VoiceToText

Code **copié puis adapté** (pas de dépendance entre les dépôts : chacun évolue
à son rythme).

| Source VoiceToText | Destination | Adaptation |
|---|---|---|
| `crates/vtt-llm/src/lib.rs` (client, erreurs HTTP, pistes de correction, `/models`, désamorçage des balises, lecture de l'`usage`) | `pde-ai` (découpé : `client`, `errors`, `hints`, `models`, `sanitize`) | Prompts de dictée retirés ; ajout du flux SSE et du JSON strict |
| `vtt-core::config` (`LlmConfig`, `LlmProfile`, `Pricing`, `PromptPreset`, contrôle de l'adresse, envoi de la clé seulement en HTTPS ou vers ce PC) | `pde-core::settings::ai` | Prompts par action IA |
| `vtt-core::usage` (jetons, coût par jour) | `pde-core::usage` | Plafond mensuel |
| `vtt-platform` `secrets_windows.rs`, `secrets_linux.rs` | `pde-platform::secrets` | `protect` / `unprotect` derrière `DataProtector` (v0.4) ; trousseau des clés API avec l'IA, préfixe `PdfEditor/` |
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
