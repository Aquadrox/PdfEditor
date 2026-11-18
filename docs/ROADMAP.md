# Roadmap et avancement

Légende : ✅ fait · 🚧 en cours · ⬜ à faire. Les identifiants renvoient à `FONCTIONS.md`.

Chaque version est livrée par une ou plusieurs PR, chacune avec ses tests ;
une version n'est terminée que lorsque la CI est verte et que la fonction a
été essayée sur le PC (Windows).

## v0.0 — Fondations ✅

- ✅ Fonctions (`FONCTIONS.md`), architecture (`ARCHITECTURE.md`), design (`DESIGN.md`), règles de contribution
- ✅ Workspace Cargo, règles de lint communes, crate `pde-core` (unités, géométrie) avec tests
- ✅ CI GitHub Actions : `linux` (format, clippy, tests), `windows`, `secrets` (gitleaks)
- ✅ Branche `main` créée et protégée (PR obligatoire, jobs `linux`, `windows`, `secrets` requis)
- ✅ Maquette des écrans principaux validée (`docs/maquette/`, voir `DESIGN.md`)

## v0.1 — Visionneuse ✅

Première version publiée : v0.0.5 (16/11/2026), installeur Windows signé et mises à jour automatiques.

- ✅ `cargo-deny` dans la CI (`deny.toml`) : aucune vulnérabilité connue, licences compatibles avec MIT, sources crates.io uniquement
- ✅ `pde-pdf` : PDFium 8125 figé et vérifié (SHA-256, `scripts/fetch-pdfium.sh`) ; thread moteur dédié ; ouverture, mot de passe, rendu, texte avec positions, plan (V-01, V-02, V-03)
- ✅ PDF de référence générés (`crates/pde-pdf/tests/fixtures/make_fixtures.py`) : pages tournées, plan, chiffrement AES-256, fichiers invalides
- ➡️ Tests de rendu comparés à des images de référence : reportés en v0.2 (aujourd'hui : contrôles de pixels ciblés)
- ✅ `pde-app` : visionneuse testable sans interface (faux moteur), cache des pages limité en mémoire, PNG
- ✅ `apps/desktop` : fenêtre principale selon la maquette, protocole `pde://`, zoom, défilement continu, miniatures, plan, aller à la page, mot de passe, glisser-déposer, ouverture en argument (V-01 à V-04)
- ✅ Tests de l'interface : modules JS (`node --test`) et parcours dans Chromium avec Tauri simulé (Playwright)
- ✅ Couche texte : sélection, copie, recherche insensible à la casse et aux accents, positions correctes sur les pages tournées (V-05)
- ✅ `pde-core::settings` (`config.toml` tolérant, valeurs bornées, raccourcis vérifiés) + page Réglages : Général, Affichage, Raccourcis clavier, Diagnostic, appliqués à chaud (R-01, R-02, R-08, R-09)
- ✅ `pde-platform` : dossiers de l'application (`PDE_HOME` pour les tests) ; le trousseau arrive avec la signature (v0.4)
- ✅ Documents récents sur l'accueil, rouvrir le dernier document au démarrage (V-01)
- ✅ Journal repris de VoiceToText (purge automatique, niveau réglable, paniques notées), sans nom ni contenu de document
- ✅ Installeur Windows (NSIS, PDFium inclus) construit par la CI ; version vérifiée contre le tag ; `CHANGELOG.md` contrôlé (`changelog.py`) ; `release.sh` (repris de VoiceToText)
- ✅ Mises à jour automatiques (repris de VoiceToText) : vérification au démarrage puis toutes les 6 h (réglable), bandeau avec nouveautés, installation sur clic après vérification de la signature minisign et de la version signée ; release publiée par la CI avec `latest.json`
- ✅ Clé de signature des mises à jour : générée par le propriétaire (identifiant `D6F1230550475C66`), clé publique dans `tauri.conf.json`, clé privée dans les secrets GitHub

## v0.2 — Édition : ajouts ✅

- ✅ Garde-fou Android dans la CI : `pde-core`, `pde-app`, `pde-pdf` et `pde-platform` compilent pour Android (aarch64), pour ne jamais fermer la porte à la v1.1
- ✅ Tests de rendu comparés à des images de référence (`crates/pde-pdf/tests/fixtures/expected/`, `PDE_UPDATE_EXPECTED=1` pour les régénérer)
- ✅ Modèle d'édition (`pde-core::edit`) : éléments ajoutés, historique annuler / rétablir, suivi « modifié », coupure des lignes ; le moteur repart du PDF d'origine et repose les éléments à chaque changement
- ✅ Sauvegarde automatique et récupération après plantage (F-02) : ajouts (textes, images) mis en lieu sûr à l'intervalle réglé, jamais le PDF ni un document protégé par mot de passe ; au lancement suivant, Récupérer / Plus tard / Ignorer ; reprise refusée si le fichier a changé (empreinte SHA-256)
- ✅ Mode Éditer : barre d'outils, couche d'édition sur les pages, panneau Propriétés (maquette `Main.dc.html`), annuler / rétablir (Ctrl+Z / Ctrl+Y), pastille « modifié », raccourcis réglables ; tests de parcours Playwright
- ✅ Zones de texte (E-01) : création par un clic et saisie directe sur la page, police, taille, gras, italique, couleur, alignement, déplacement et largeur à la souris ou en mm, dupliquer, supprimer ; polices standard du PDF pour l'alphabet latin d'Europe de l'Ouest, Noto embarquée (`pde-fonts`) dès qu'une zone sort de ce jeu (latin étendu, grec, cyrillique), une fois par variante ; caractères non couverts (chinois, arabe…) signalés dans le panneau. Reste pour plus tard : sous-ensembles de polices (taille du fichier), écritures complexes (arabe, hébreu, indiennes) et CJK
- ✅ Images (E-02) : PNG, JPEG, WebP par le bouton Image, le presse-papiers (Ctrl+V) ou un glisser-déposer ; JPEG inclus sans recompression, transparence gardée, orientation EXIF appliquée ; coins avec proportions verrouillables, rotation par quarts de tour, opacité ; tests de rendu (image de référence `images.png`)
- ✅ Sélection, déplacement, alignement, magnétisme (E-03) : Maj+clic, Ctrl+A (page), Tab / Maj+Tab ; déplacer, décaler aux flèches, dupliquer, supprimer toute la sélection en une opération (`Operation::Batch`, une annulation) ; aligner (bords, centres) et répartir ; magnétisme sur les bords et centres des autres éléments et de la page, grille réglable, repères affichés, Alt pour s'en passer
- ✅ Annotations et formes (E-04) : surligner, souligner, barrer le texte sélectionné ; dessin à main levée ; rectangle, ellipse, trait, flèche (Maj : carré, cercle, 45°) ; couleur, épaisseur, remplissage ; poignées ; tracés dans le contenu de la page (même rendu partout), image de référence `annotations.png` ; notes = vraies annotations PDF « Texte » (commentaire, auteur, icône) lisibles dans les autres lecteurs
- ✅ Enregistrer / Enregistrer sous (F-01) : contenu relu avant d'écrire, remplacement atomique, mot de passe conservé ; question avant de fermer le document ou la fenêtre, ou d'en ouvrir un autre, avec des modifications
- ✅ Réglages Édition et Fichiers (R-03, R-04) : nom d'auteur des notes, police, taille et couleur des nouvelles zones de texte, couleur de surlignage, magnétisme et grille ; intervalle de la sauvegarde automatique, dossier proposé par les dialogues Ouvrir et Image, suffixe des copies (« Enregistrer sous » propose « contrat (modifié).pdf » à côté de l'original). Le comportement à l'enregistrement d'un document signé viendra avec la signature (v0.4)

## v0.3 — Contenu existant et pages ✅

- ✅ Texte du document (E-05) : outil « Texte du document » en mode Éditer, chaque ligne de la page (objets texte d'une même ligne, même police, corps et couleur) se récrit à sa place, ligne par ligne ; la police du document est gardée si elle sait écrire le nouveau texte (police fournie par le lecteur, ou seulement des caractères déjà écrits avec cette police embarquée, vérifié en relisant le texte écrit), sinon la ligne est récrite avec la police standard la plus proche (Noto au-delà du latin) et le panneau le signale ; vider le texte supprime la ligne ; désignée par le rang de ses objets et vérifiée par sa place d'origine. Restent : paragraphes sur plusieurs lignes (reflux), pages tournées, texte tourné ou dans un formulaire XObject, texte invisible (couche OCR)
- ✅ Images d'origine (E-06) : en mode Éditer, les images du document se choisissent d'un clic, se déplacent et se redimensionnent (proportions gardées), se suppriment ou se remplacent (« Remplacer… » : nouvelle image dans le même cadre, en une opération) ; l'image d'origine garde son contenu (aucune recompression), seul son cadre change ; désignée par son rang dans la page et vérifiée par son cadre d'origine à chaque pose. Restent : images à l'intérieur d'un formulaire XObject, recadrage du contenu de l'image
- ✅ Caviardage réel (E-07) : outil Caviarder (texte sélectionné ou zone tracée) ; tout objet qui touche une zone (texte, image, tracé, formulaire XObject) est retiré du contenu et remplacé par son image où la zone est noircie, les annotations touchées sont supprimées, un aplat noir couvre la zone ; appliqué à chaque pose comme le reste, confirmation avant d'enregistrer ; tests PDFium (texte absent du fichier et de ses octets, page tournée). Restent : le texte non caviardé d'une ligne touchée devient une image (plus sélectionnable) ; métadonnées, signets et pièces jointes ne sont pas parcourus
- ✅ Pages (P-01, P-02) : mode Pages, grille des pages ; page blanche, dupliquer, pivoter, supprimer, réordonner en glissant (clavier : Suppr, Ctrl+A, flèches, Entrée) ; une opération = une entrée d'historique, les ajouts suivent leur page ; signets renvoyés à la nouvelle place ; plan des pages repris par la sauvegarde automatique. Restent : rotation des pages portant du texte ajouté, souligné ou barré ; signets et liens qui visent une page déplacée hors de la plus longue suite gardée (recopiée)
- ✅ Pages d'autres PDF (P-03) : « Insérer un PDF » (plusieurs fichiers, en une opération) ou PDF glissés sur la grille, à l'endroit du dépôt ; fusionner = insérer à la fin ; « Extraire » écrit les pages choisies, avec leurs ajouts, dans un nouveau PDF (nom proposé « contrat (pages 2-3).pdf », mot de passe conservé) sans changer le document ; la sauvegarde automatique désigne les PDF insérés par leur chemin et leur empreinte, sans les copier. Restent : insertion d'un PDF protégé par mot de passe (refusée avec un message), signets et champs de formulaire des PDF insérés
- ✅ Formulaires (E-08) : en mode Lire, champs texte, cases, boutons radio et listes se remplissent sur la page ; valeurs écrites dans le fichier (`lopdf`, apparence redessinée par PDFium), annulables et reprises par la sauvegarde automatique ; « Aplatir le formulaire » fige les champs dans la page. Restent : listes modifiables traitées comme des listes fermées, champs de signature et boutons poussoirs ignorés, PDF protégés par mot de passe (refusés), champs des pages recopiées ou insérées
- ✅ Propriétés du document (E-11) : titre, auteur, sujet, mots-clés depuis le menu (« Propriétés du document… »), écrits dans le dictionnaire d'information. Reste : métadonnées XMP non mises à jour
- ✅ Tests de parcours de l'interface (Playwright) : texte et images d'origine, caviardage, pages et PDF insérés, formulaires et propriétés

## v0.4 — Signature et verrouillage 🚧

Livraisons : v0.0.19 (signer, verrouiller, vérifier), v0.0.20 (enregistrement des documents signés, signature manuscrite, horodatage).

- ✅ `pde-platform` : protection des clés privées par le système (repris de VoiceToText : DPAPI sous Windows, clé gardée dans le trousseau de la session sous Linux), derrière le trait `DataProtector` de `pde-core`. Le trousseau pour d'autres secrets (clés API) viendra avec l'IA
- ✅ `pde-sign` (Rust pur, RustCrypto + `lopdf`) : mise à jour incrémentale écrite octet par octet (table `xref` ou flux de références croisées, selon le fichier), signature PAdES B-B (CMS détaché, `signingCertificateV2`, SHA-256, ECDSA P-256 / P-384 ou RSA) (S-05) ; chaque test contre-vérifié par `pdfsig` (Poppler) et `qpdf --check`, exigés par la CI
- ✅ Identités : certificat auto-signé généré (P-256, 5 ans, usages signature et non-répudiation), import PKCS#12 (RSA, ECDSA) ; clé privée chiffrée par le système dans le dossier de données, jamais en clair (S-01, S-02). Écart assumé : le mot de passe du `.p12` n'est demandé qu'à l'import, la clé est ensuite protégée comme celle d'un certificat généré
- ✅ Apparence (S-03) : signature invisible, ou visible dans un cadre tracé sur la page : signature manuscrite dessinée (souris, stylet), tapée (Caveat, Dancing Script, Homemade Apple) ou importée (papier clair rendu transparent), détourée, en image PDF avec transparence au-dessus du nom, de l'organisation et de la date ; « Mes signatures » (modèles enregistrés). Reste : texte droit sur une page tournée
- 🚧 Champs (S-04) : signer un champ de signature vide existant d'un clic ; nouveau champ dans le cadre tracé (CropBox et rotation de la page pris en compte). Reste : placer des champs vides pour d'autres signataires (avec le cycle, v0.8)
- ✅ Verrouillage DocMDP (S-06) : « Laisser modifiable », « Verrouiller, sauf formulaires et signatures » (`/P 2`), « Verrouiller entièrement » (`/P 1`) ; seule la première signature verrouille ; un document verrouillé passe en lecture (ni Éditer, ni Pages, ni formulaire) et refuse toute nouvelle signature
- 🚧 Panneau Signatures et sceau (S-07) : chaque champ, signé ou vide ; intégrité des octets signés, signature, certificat désigné, document complété depuis, verrouillage ; sceau de cire dans la barre du haut. Restent : chaîne de confiance (certificats reconnus du système), analyse de ce qui a changé après une signature
- ✅ Enregistrement incrémental des documents signés (F-03) : le document réécrit par PDFium est comparé au fichier signé, objet par objet et sur le fond (ordre des clés, compression ignorés) ; seuls les objets changés sont ajoutés à la fin du fichier ; les dictionnaires de signature ne sont jamais réécrits ; refusé si une signature valide cessait de l'être. Verrou « formulaires » (`/P 2`) : seuls les champs et d'autres signatures restent permis dans l'interface
- ✅ Horodatage RFC 3161 (S-08, PAdES B-T) : jeton du serveur ajouté au CMS (attribut `timeStampToken`), vérifié à la réception (empreinte, nonce, signature du serveur) et à la lecture ; essayé avec le serveur public de DigiCert. Serveurs en `http://` seulement (le jeton est signé ; pas de pile TLS à embarquer, compilation Android préservée)
- ✅ Réglages Signature (R-05) : horodatage par défaut, serveur d'horodatage, verrouillage proposé. Reste : identité par défaut (aujourd'hui la dernière utilisée)
- ⬜ Signature d'un document protégé par mot de passe (refusée avec un message)
- ⬜ Analyse de ce qui a changé après une signature (permis ou non par DocMDP), chaîne de confiance des certificats

## v0.5 — IA : socle et analyse

- ⬜ `pde-ai` : client repris de VoiceToText, découpé et testé ; trait `ChatModel` et faux modèle de test ; réponses en flux
- ⬜ Réglages IA : préréglages, profils, clé dans le trousseau, test des réglages, profil de secours, prompts éditables (IA-01, IA-02, IA-11, R-07)
- ⬜ Estimation du coût, confirmation, consommation, plafond (IA-03)
- ⬜ Panneau IA : analyse sur sélection / pages / document (IA-04)
- ⬜ Questions sur le document avec citations de pages (IA-05)
- ⬜ Catalogue des actions (`pde-app::actions`) : chaque action de l'éditeur décrite une seule fois (nom, explication, paramètres en JSON Schema, contrôle, exécution annulable) ; il sert à la palette de commandes, à l'aide « Comment faire ? », aux outils de l'IA et plus tard au serveur MCP — une action ajoutée à l'éditeur est aussitôt connue de tous
- ⬜ Palette de commandes Ctrl+K et « Comment faire ? », sans IA (V-08)
- ⬜ Assistant d'actions (IA-15) : sélection + demande → appel d'outils de l'API compatible OpenAI (`tools`) → actions proposées en aperçu, Appliquer / Refuser ; ou explication avec l'outil montré ; repli sur « Comment faire ? » avec un modèle qui n'appelle pas d'outils
- ⬜ Recettes intégrées et personnelles (IA-16)

## v0.6 — IA : traduction

- ⬜ Traduction par blocs, mise en page conservée, ajustement de la taille (IA-06)
- ⬜ Modes : document traduit, bilingue, texte seul (IA-07)
- ⬜ Glossaires (IA-08)

## v0.7 — IA : modification

- ⬜ Modification par consigne, différences, acceptation bloc par bloc, annulable (IA-09)
- ⬜ Anonymisation assistée (IA-10)
- ⬜ Annulation d'une action en cours (IA-12)
- ⬜ Serveur MCP local optionnel, sur le catalogue des actions (IA-17)

## v0.8 — Cycle de signature

- ⬜ `pde-workflow` : plan, machine à états, manifeste embarqué, journal d'audit (C-01, C-02, C-05)
- ⬜ Assistant de création du cycle, champs attribués (C-01)
- ⬜ « À vous de signer », envoi au suivant, suivi, refus (C-03, C-04)
- ⬜ Clôture et verrouillage (C-06), modèles de cycles (C-07)

## v0.9 — Finition et distribution

- ⬜ Version Linux (AppImage)
- ⬜ Accessibilité (clavier complet, lecteurs d'écran, contrastes), performances sur gros documents
- ⬜ Manuel d'utilisation
- ⬜ Fonctions S restantes : onglets (V-06), tampons (E-09), recadrage (P-04), images → PDF (P-05), export (F-04), mot de passe (F-05), impression (F-06)

## v1.0 — Version stable

- ⬜ Toutes les fonctions **M** livrées, testées, documentées

## v1.1 — Android

Même code Rust et même interface (Tauri 2 en version mobile), adaptée au
tactile. Fonctions A-01 à A-05 de `FONCTIONS.md`.

- ⬜ Application Android Tauri 2 (arm64), PDFium pour Android figé et vérifié (SHA-256)
- ⬜ Interface tactile : panneaux en tiroirs, barre d'outils compacte, pincer pour zoomer, sélection au doigt (A-01)
- ⬜ Fichiers : sélecteur de documents Android, « Ouvrir avec » et « Partager vers PdfEditor », enregistrement (A-02)
- ⬜ Lecture, recherche, annotations, signature et IA ; secrets dans le Keystore Android (A-03)
- ⬜ Mises à jour : APK signé (clé du propriétaire dans les secrets GitHub), manifeste `latest-android.json` avec taille et SHA-256, comme VoiceToText (A-04)
- ⬜ CI : APK de test à chaque PR, APK signé sur un tag ; tests sur émulateur des parcours principaux (A-05)

## Plus tard

OCR (IA-13), remplissage assisté (IA-14), PKCS#11 et magasin Windows (S-09),
PAdES B-LT / B-LTA (S-10), transports de cycle (C-08), en-têtes et pieds de
page (E-10), compression (P-06), mode présentation (V-07), macOS, comparaison
de deux versions d'un document.
